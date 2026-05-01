//
// Copyright 2026 Hans W. Uhlig, IBM. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use nanograph_util::{CompressionAlgorithm, EncryptionAlgorithm, IntegrityAlgorithm};
use nanograph_vfs::MemoryFileSystem;
use nanograph_wal::{
    Durability, HEADER_SIZE, LogSequenceNumber, WriteAheadLogConfig, WriteAheadLogManager,
    WriteAheadLogRecord,
};
use std::hint::black_box;

/// Configuration for benchmarking
#[derive(Clone, Debug)]
struct BenchConfig {
    name: String,
    integrity: IntegrityAlgorithm,
    compression: CompressionAlgorithm,
    encryption: EncryptionAlgorithm,
}

impl BenchConfig {
    fn new(
        integrity: IntegrityAlgorithm,
        compression: CompressionAlgorithm,
        encryption: EncryptionAlgorithm,
    ) -> Self {
        let name = format!(
            "{}-{}-{}",
            integrity.name(),
            compression.name(),
            encryption.name()
        );
        Self {
            name,
            integrity,
            compression,
            encryption,
        }
    }

    fn to_wal_config(&self) -> WriteAheadLogConfig {
        let encryption_key = if self.encryption != EncryptionAlgorithm::None {
            Some(self.encryption.generate_key())
        } else {
            None
        };

        WriteAheadLogConfig {
            shard_id: 1,
            max_segment_size: 100 * 1024 * 1024,
            sync_on_rotate: false,
            checksum: self.integrity,
            compression: self.compression,
            encryption: self.encryption,
            encryption_key,
        }
    }
}

/// Generate all combinations of algorithms for comprehensive testing
fn all_configs() -> Vec<BenchConfig> {
    let mut configs = Vec::new();

    // Baseline: no protection
    configs.push(BenchConfig::new(
        IntegrityAlgorithm::None,
        CompressionAlgorithm::None,
        EncryptionAlgorithm::None,
    ));

    // Integrity only
    configs.push(BenchConfig::new(
        IntegrityAlgorithm::Crc32c,
        CompressionAlgorithm::None,
        EncryptionAlgorithm::None,
    ));
    configs.push(BenchConfig::new(
        IntegrityAlgorithm::XXHash32,
        CompressionAlgorithm::None,
        EncryptionAlgorithm::None,
    ));

    // Compression only
    configs.push(BenchConfig::new(
        IntegrityAlgorithm::None,
        CompressionAlgorithm::Lz4,
        EncryptionAlgorithm::None,
    ));
    configs.push(BenchConfig::new(
        IntegrityAlgorithm::None,
        CompressionAlgorithm::Zstd,
        EncryptionAlgorithm::None,
    ));
    configs.push(BenchConfig::new(
        IntegrityAlgorithm::None,
        CompressionAlgorithm::Snappy,
        EncryptionAlgorithm::None,
    ));

    // Encryption only
    configs.push(BenchConfig::new(
        IntegrityAlgorithm::None,
        CompressionAlgorithm::None,
        EncryptionAlgorithm::Aes256Gcm,
    ));
    configs.push(BenchConfig::new(
        IntegrityAlgorithm::None,
        CompressionAlgorithm::None,
        EncryptionAlgorithm::ChaCha20Poly1305,
    ));

    // Integrity + Compression (common combinations)
    configs.push(BenchConfig::new(
        IntegrityAlgorithm::Crc32c,
        CompressionAlgorithm::Lz4,
        EncryptionAlgorithm::None,
    ));
    configs.push(BenchConfig::new(
        IntegrityAlgorithm::Crc32c,
        CompressionAlgorithm::Zstd,
        EncryptionAlgorithm::None,
    ));
    configs.push(BenchConfig::new(
        IntegrityAlgorithm::XXHash32,
        CompressionAlgorithm::Snappy,
        EncryptionAlgorithm::None,
    ));

    // Integrity + Encryption (common combinations)
    configs.push(BenchConfig::new(
        IntegrityAlgorithm::Crc32c,
        CompressionAlgorithm::None,
        EncryptionAlgorithm::Aes256Gcm,
    ));
    configs.push(BenchConfig::new(
        IntegrityAlgorithm::XXHash32,
        CompressionAlgorithm::None,
        EncryptionAlgorithm::ChaCha20Poly1305,
    ));

    // Compression + Encryption (common combinations)
    configs.push(BenchConfig::new(
        IntegrityAlgorithm::None,
        CompressionAlgorithm::Lz4,
        EncryptionAlgorithm::Aes256Gcm,
    ));
    configs.push(BenchConfig::new(
        IntegrityAlgorithm::None,
        CompressionAlgorithm::Zstd,
        EncryptionAlgorithm::ChaCha20Poly1305,
    ));

    // Full protection (all three)
    configs.push(BenchConfig::new(
        IntegrityAlgorithm::Crc32c,
        CompressionAlgorithm::Lz4,
        EncryptionAlgorithm::Aes256Gcm,
    ));
    configs.push(BenchConfig::new(
        IntegrityAlgorithm::XXHash32,
        CompressionAlgorithm::Zstd,
        EncryptionAlgorithm::ChaCha20Poly1305,
    ));
    configs.push(BenchConfig::new(
        IntegrityAlgorithm::Crc32c,
        CompressionAlgorithm::Snappy,
        EncryptionAlgorithm::Aes256Gcm,
    ));

    configs
}

fn bench_wal_append_with_configs(c: &mut Criterion) {
    let mut group = c.benchmark_group("wal_append_configs");
    let payload = vec![42u8; 1024];

    for config in all_configs() {
        group.throughput(Throughput::Bytes(payload.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("1kb", &config.name),
            &config,
            |b, config| {
                let fs = MemoryFileSystem::new();
                let wal_config = config.to_wal_config();
                let manager = WriteAheadLogManager::new(fs, "/wal", wal_config).unwrap();
                let mut writer = manager.writer().unwrap();

                b.iter(|| {
                    let record = WriteAheadLogRecord {
                        kind: 1,
                        payload: black_box(&payload),
                    };
                    writer.append(record, Durability::None).unwrap();
                });
            },
        );
    }

    group.finish();
}

fn bench_wal_append_varying_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("wal_append_sizes");

    // Test with a few representative configurations
    let configs = vec![
        BenchConfig::new(
            IntegrityAlgorithm::None,
            CompressionAlgorithm::None,
            EncryptionAlgorithm::None,
        ),
        BenchConfig::new(
            IntegrityAlgorithm::Crc32c,
            CompressionAlgorithm::Lz4,
            EncryptionAlgorithm::Aes256Gcm,
        ),
    ];

    for size in [64, 256, 1024, 4096, 16384].iter() {
        for config in &configs {
            group.throughput(Throughput::Bytes(*size as u64));
            group.bench_with_input(
                BenchmarkId::new(&config.name, size),
                &(*size, config),
                |b, &(size, config)| {
                    let fs = MemoryFileSystem::new();
                    let wal_config = config.to_wal_config();
                    let manager = WriteAheadLogManager::new(fs, "/wal", wal_config).unwrap();
                    let mut writer = manager.writer().unwrap();
                    let payload = vec![42u8; size];

                    b.iter(|| {
                        let record = WriteAheadLogRecord {
                            kind: 1,
                            payload: black_box(&payload),
                        };
                        writer.append(record, Durability::None).unwrap();
                    });
                },
            );
        }
    }

    group.finish();
}

fn bench_wal_read_with_configs(c: &mut Criterion) {
    let mut group = c.benchmark_group("wal_read_configs");
    let count = 1000;
    group.throughput(Throughput::Elements(count as u64));

    for config in all_configs() {
        group.bench_with_input(
            BenchmarkId::new("1000_records", &config.name),
            &config,
            |b, config| {
                let fs = MemoryFileSystem::new();
                let wal_config = config.to_wal_config();
                let manager = WriteAheadLogManager::new(fs, "/wal", wal_config).unwrap();
                let mut writer = manager.writer().unwrap();

                // Write records
                for i in 0..count {
                    let payload = format!("Record {}", i);
                    let record = WriteAheadLogRecord {
                        kind: 1,
                        payload: payload.as_bytes(),
                    };
                    writer.append(record, Durability::None).unwrap();
                }

                let start_lsn = LogSequenceNumber {
                    segment_id: 0,
                    offset: HEADER_SIZE as u64, // Skip header
                };

                b.iter(|| {
                    let mut reader = manager.reader_from(start_lsn).unwrap();
                    let mut read_count = 0;
                    while let Some(_entry) = reader.next().unwrap() {
                        read_count += 1;
                    }
                    black_box(read_count);
                });
            },
        );
    }

    group.finish();
}

fn bench_wal_batch_append_with_configs(c: &mut Criterion) {
    let mut group = c.benchmark_group("wal_batch_append_configs");
    let batch_size = 100;
    group.throughput(Throughput::Elements(batch_size as u64));

    // Test with representative configurations
    let configs = vec![
        BenchConfig::new(
            IntegrityAlgorithm::None,
            CompressionAlgorithm::None,
            EncryptionAlgorithm::None,
        ),
        BenchConfig::new(
            IntegrityAlgorithm::Crc32c,
            CompressionAlgorithm::None,
            EncryptionAlgorithm::None,
        ),
        BenchConfig::new(
            IntegrityAlgorithm::None,
            CompressionAlgorithm::Lz4,
            EncryptionAlgorithm::None,
        ),
        BenchConfig::new(
            IntegrityAlgorithm::Crc32c,
            CompressionAlgorithm::Lz4,
            EncryptionAlgorithm::Aes256Gcm,
        ),
    ];

    for config in configs {
        group.bench_with_input(
            BenchmarkId::new("100_records", &config.name),
            &config,
            |b, config| {
                let fs = MemoryFileSystem::new();
                let wal_config = config.to_wal_config();
                let manager = WriteAheadLogManager::new(fs, "/wal", wal_config).unwrap();
                let mut writer = manager.writer().unwrap();

                let payloads: Vec<_> = (0..batch_size).map(|i| format!("Record {}", i)).collect();

                b.iter(|| {
                    let records: Vec<_> = payloads
                        .iter()
                        .map(|p| WriteAheadLogRecord {
                            kind: 1,
                            payload: p.as_bytes(),
                        })
                        .collect();

                    writer
                        .append_batch(records.into_iter(), Durability::None)
                        .unwrap();
                });
            },
        );
    }

    group.finish();
}

fn bench_wal_compression_effectiveness(c: &mut Criterion) {
    let mut group = c.benchmark_group("wal_compression_effectiveness");

    // Test with highly compressible data
    let compressible_data = vec![42u8; 4096];
    // Test with random (incompressible) data
    let random_data: Vec<u8> = (0..4096).map(|i| (i * 7 + 13) as u8).collect();

    let compression_configs = vec![
        BenchConfig::new(
            IntegrityAlgorithm::None,
            CompressionAlgorithm::None,
            EncryptionAlgorithm::None,
        ),
        BenchConfig::new(
            IntegrityAlgorithm::None,
            CompressionAlgorithm::Lz4,
            EncryptionAlgorithm::None,
        ),
        BenchConfig::new(
            IntegrityAlgorithm::None,
            CompressionAlgorithm::Zstd,
            EncryptionAlgorithm::None,
        ),
        BenchConfig::new(
            IntegrityAlgorithm::None,
            CompressionAlgorithm::Snappy,
            EncryptionAlgorithm::None,
        ),
    ];

    for (data_type, data) in [
        ("compressible", &compressible_data),
        ("random", &random_data),
    ] {
        for config in &compression_configs {
            group.throughput(Throughput::Bytes(data.len() as u64));
            group.bench_with_input(
                BenchmarkId::new(format!("{}_{}", data_type, config.name), data.len()),
                &(data, config),
                |b, &(data, config)| {
                    let fs = MemoryFileSystem::new();
                    let wal_config = config.to_wal_config();
                    let manager = WriteAheadLogManager::new(fs, "/wal", wal_config).unwrap();
                    let mut writer = manager.writer().unwrap();

                    b.iter(|| {
                        let record = WriteAheadLogRecord {
                            kind: 1,
                            payload: black_box(data),
                        };
                        writer.append(record, Durability::None).unwrap();
                    });
                },
            );
        }
    }

    group.finish();
}

fn bench_wal_append_with_durability(c: &mut Criterion) {
    let mut group = c.benchmark_group("wal_append_durability");
    let payload = vec![42u8; 1024];

    // Test durability with a representative configuration
    let config = BenchConfig::new(
        IntegrityAlgorithm::Crc32c,
        CompressionAlgorithm::Lz4,
        EncryptionAlgorithm::None,
    );

    for durability in [Durability::None, Durability::Buffered, Durability::Sync].iter() {
        let name = match durability {
            Durability::None => "none",
            Durability::Buffered => "buffered",
            Durability::Sync => "sync",
        };

        group.bench_function(name, |b| {
            let fs = MemoryFileSystem::new();
            let wal_config = config.to_wal_config();
            let manager = WriteAheadLogManager::new(fs, "/wal", wal_config).unwrap();
            let mut writer = manager.writer().unwrap();

            b.iter(|| {
                let record = WriteAheadLogRecord {
                    kind: 1,
                    payload: black_box(&payload),
                };
                writer.append(record, *durability).unwrap();
            });
        });
    }

    group.finish();
}

fn bench_wal_sequential_read_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("wal_sequential_read_write");

    // Test with representative configurations
    let configs = vec![
        BenchConfig::new(
            IntegrityAlgorithm::None,
            CompressionAlgorithm::None,
            EncryptionAlgorithm::None,
        ),
        BenchConfig::new(
            IntegrityAlgorithm::Crc32c,
            CompressionAlgorithm::Lz4,
            EncryptionAlgorithm::Aes256Gcm,
        ),
    ];

    for config in configs {
        group.bench_with_input(
            BenchmarkId::new("interleaved", &config.name),
            &config,
            |b, config| {
                let fs = MemoryFileSystem::new();
                let wal_config = config.to_wal_config();
                let manager = WriteAheadLogManager::new(fs, "/wal", wal_config).unwrap();
                let mut writer = manager.writer().unwrap();

                b.iter(|| {
                    // Write
                    let payload = b"test data";
                    let record = WriteAheadLogRecord {
                        kind: 1,
                        payload: black_box(payload),
                    };
                    let lsn = writer.append(record, Durability::None).unwrap();

                    // Read
                    let mut reader = manager.reader_from(lsn).unwrap();
                    let entry = reader.next().unwrap();
                    black_box(entry);
                });
            },
        );
    }

    group.finish();
}

fn bench_wal_concurrent_writes(c: &mut Criterion) {
    let mut group = c.benchmark_group("wal_concurrent_writes");

    // Test with representative configurations
    let configs = vec![
        BenchConfig::new(
            IntegrityAlgorithm::None,
            CompressionAlgorithm::None,
            EncryptionAlgorithm::None,
        ),
        BenchConfig::new(
            IntegrityAlgorithm::Crc32c,
            CompressionAlgorithm::Lz4,
            EncryptionAlgorithm::None,
        ),
    ];

    for config in configs {
        group.bench_with_input(
            BenchmarkId::new("4_threads", &config.name),
            &config,
            |b, config| {
                b.iter(|| {
                    let fs = MemoryFileSystem::new();
                    let wal_config = config.to_wal_config();
                    let manager = WriteAheadLogManager::new(fs, "/wal", wal_config).unwrap();

                    let handles: Vec<_> = (0..4)
                        .map(|i| {
                            let mut writer = manager.writer().unwrap();
                            std::thread::spawn(move || {
                                for j in 0..25 {
                                    let payload = format!("Thread {} Record {}", i, j);
                                    let record = WriteAheadLogRecord {
                                        kind: i as u16,
                                        payload: payload.as_bytes(),
                                    };
                                    writer.append(record, Durability::None).unwrap();
                                }
                            })
                        })
                        .collect();

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_wal_append_with_configs,
    bench_wal_append_varying_sizes,
    bench_wal_read_with_configs,
    bench_wal_batch_append_with_configs,
    bench_wal_compression_effectiveness,
    bench_wal_append_with_durability,
    bench_wal_sequential_read_write,
    bench_wal_concurrent_writes
);

criterion_main!(benches);
