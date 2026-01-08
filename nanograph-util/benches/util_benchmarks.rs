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

//! Comprehensive benchmarks for nanograph-util

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use nanograph_util::*;
use std::hint::black_box;

// ============================================================================
// Test Data Generation
// ============================================================================

fn generate_test_data(size: usize, pattern: DataPattern) -> Vec<u8> {
    match pattern {
        DataPattern::Zeros => vec![0u8; size],
        DataPattern::Ones => vec![255u8; size],
        DataPattern::Random => (0..size).map(|i| ((i * 7919) % 256) as u8).collect(),
        DataPattern::Repetitive => {
            let pattern = b"Hello, World! This is a repetitive pattern. ";
            pattern.iter().cycle().take(size).copied().collect()
        }
        DataPattern::Sequential => (0..size).map(|i| (i % 256) as u8).collect(),
    }
}

#[derive(Clone, Copy)]
enum DataPattern {
    Zeros,
    Ones,
    Random,
    Repetitive,
    Sequential,
}

impl DataPattern {
    fn name(&self) -> &'static str {
        match self {
            DataPattern::Zeros => "zeros",
            DataPattern::Ones => "ones",
            DataPattern::Random => "random",
            DataPattern::Repetitive => "repetitive",
            DataPattern::Sequential => "sequential",
        }
    }
}

// ============================================================================
// Compression Benchmarks
// ============================================================================

fn bench_compression_algorithms(c: &mut Criterion) {
    let sizes = [1024, 4096, 16384, 65536, 262144]; // 1KB to 256KB
    let patterns = [
        DataPattern::Zeros,
        DataPattern::Random,
        DataPattern::Repetitive,
    ];

    for &size in &sizes {
        for &pattern in &patterns {
            let data = generate_test_data(size, pattern);
            let group_name = format!("compression/{}/{}", pattern.name(), size);
            let mut group = c.benchmark_group(group_name);
            group.throughput(Throughput::Bytes(size as u64));

            // Benchmark None (baseline)
            group.bench_function("none/compress", |b| {
                b.iter(|| {
                    CompressionAlgorithm::None
                        .compress(black_box(&data))
                        .unwrap()
                })
            });

            // Benchmark LZ4
            group.bench_function("lz4/compress", |b| {
                b.iter(|| {
                    CompressionAlgorithm::Lz4
                        .compress(black_box(&data))
                        .unwrap()
                })
            });

            let compressed_lz4 = CompressionAlgorithm::Lz4.compress(&data).unwrap();
            group.bench_function("lz4/decompress", |b| {
                b.iter(|| {
                    CompressionAlgorithm::Lz4
                        .decompress(black_box(&compressed_lz4), Some(size))
                        .unwrap()
                })
            });

            // Benchmark Zstd
            group.bench_function("zstd/compress", |b| {
                b.iter(|| {
                    CompressionAlgorithm::Zstd
                        .compress(black_box(&data))
                        .unwrap()
                })
            });

            let compressed_zstd = CompressionAlgorithm::Zstd.compress(&data).unwrap();
            group.bench_function("zstd/decompress", |b| {
                b.iter(|| {
                    CompressionAlgorithm::Zstd
                        .decompress(black_box(&compressed_zstd), Some(size))
                        .unwrap()
                })
            });

            // Benchmark Snappy
            group.bench_function("snappy/compress", |b| {
                b.iter(|| {
                    CompressionAlgorithm::Snappy
                        .compress(black_box(&data))
                        .unwrap()
                })
            });

            let compressed_snappy = CompressionAlgorithm::Snappy.compress(&data).unwrap();
            group.bench_function("snappy/decompress", |b| {
                b.iter(|| {
                    CompressionAlgorithm::Snappy
                        .decompress(black_box(&compressed_snappy), Some(size))
                        .unwrap()
                })
            });

            group.finish();
        }
    }
}

fn bench_compression_into_buffer(c: &mut Criterion) {
    let sizes = [1024, 16384, 65536];
    let data = generate_test_data(65536, DataPattern::Repetitive);

    let mut group = c.benchmark_group("compression/into_buffer");

    for &size in &sizes {
        let test_data = &data[..size];
        group.throughput(Throughput::Bytes(size as u64));

        for algo in [
            CompressionAlgorithm::Lz4,
            CompressionAlgorithm::Zstd,
            CompressionAlgorithm::Snappy,
        ] {
            let max_size = algo.max_compressed_size(size);
            let mut buffer = vec![0u8; max_size];

            group.bench_with_input(
                BenchmarkId::new(algo.name(), size),
                &test_data,
                |b, data| {
                    b.iter(|| {
                        algo.compress_into(black_box(data), black_box(&mut buffer))
                            .unwrap()
                    })
                },
            );
        }
    }

    group.finish();
}

// ============================================================================
// Encryption Benchmarks
// ============================================================================

fn bench_encryption_algorithms(c: &mut Criterion) {
    let sizes = [1024, 4096, 16384, 65536, 262144]; // 1KB to 256KB

    for &size in &sizes {
        let data = generate_test_data(size, DataPattern::Random);
        let group_name = format!("encryption/{}", size);
        let mut group = c.benchmark_group(group_name);
        group.throughput(Throughput::Bytes(size as u64));

        // Benchmark AES-256-GCM
        let key_aes = EncryptionAlgorithm::Aes256Gcm.generate_key();
        let nonce_aes = EncryptionAlgorithm::Aes256Gcm.generate_nonce();

        group.bench_function("aes256gcm/encrypt", |b| {
            b.iter(|| {
                EncryptionAlgorithm::Aes256Gcm
                    .encrypt(black_box(&key_aes), black_box(&nonce_aes), black_box(&data))
                    .unwrap()
            })
        });

        let encrypted_aes = EncryptionAlgorithm::Aes256Gcm
            .encrypt(&key_aes, &nonce_aes, &data)
            .unwrap();

        group.bench_function("aes256gcm/decrypt", |b| {
            b.iter(|| {
                EncryptionAlgorithm::Aes256Gcm
                    .decrypt(
                        black_box(&key_aes),
                        black_box(&nonce_aes),
                        black_box(&encrypted_aes),
                    )
                    .unwrap()
            })
        });

        // Benchmark ChaCha20-Poly1305
        let key_chacha = EncryptionAlgorithm::ChaCha20Poly1305.generate_key();
        let nonce_chacha = EncryptionAlgorithm::ChaCha20Poly1305.generate_nonce();

        group.bench_function("chacha20poly1305/encrypt", |b| {
            b.iter(|| {
                EncryptionAlgorithm::ChaCha20Poly1305
                    .encrypt(
                        black_box(&key_chacha),
                        black_box(&nonce_chacha),
                        black_box(&data),
                    )
                    .unwrap()
            })
        });

        let encrypted_chacha = EncryptionAlgorithm::ChaCha20Poly1305
            .encrypt(&key_chacha, &nonce_chacha, &data)
            .unwrap();

        group.bench_function("chacha20poly1305/decrypt", |b| {
            b.iter(|| {
                EncryptionAlgorithm::ChaCha20Poly1305
                    .decrypt(
                        black_box(&key_chacha),
                        black_box(&nonce_chacha),
                        black_box(&encrypted_chacha),
                    )
                    .unwrap()
            })
        });

        group.finish();
    }
}

fn bench_encryption_key_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("encryption/key_generation");

    group.bench_function("aes256gcm", |b| {
        b.iter(|| EncryptionAlgorithm::Aes256Gcm.generate_key())
    });

    group.bench_function("chacha20poly1305", |b| {
        b.iter(|| EncryptionAlgorithm::ChaCha20Poly1305.generate_key())
    });

    group.finish();
}

fn bench_encryption_nonce_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("encryption/nonce_generation");

    group.bench_function("aes256gcm", |b| {
        b.iter(|| EncryptionAlgorithm::Aes256Gcm.generate_nonce())
    });

    group.bench_function("chacha20poly1305", |b| {
        b.iter(|| EncryptionAlgorithm::ChaCha20Poly1305.generate_nonce())
    });

    group.finish();
}

// ============================================================================
// Integrity Benchmarks
// ============================================================================

fn bench_integrity_algorithms(c: &mut Criterion) {
    let sizes = [1024, 4096, 16384, 65536, 262144, 1048576]; // 1KB to 1MB

    for &size in &sizes {
        let data = generate_test_data(size, DataPattern::Random);
        let group_name = format!("integrity/{}", size);
        let mut group = c.benchmark_group(group_name);
        group.throughput(Throughput::Bytes(size as u64));

        // Benchmark CRC32C
        group.bench_function("crc32c/hash", |b| {
            b.iter(|| IntegrityAlgorithm::Crc32c.hash(black_box(&data)))
        });

        let hash_crc = IntegrityAlgorithm::Crc32c.hash(&data);
        group.bench_function("crc32c/verify", |b| {
            b.iter(|| {
                IntegrityAlgorithm::Crc32c
                    .verify(black_box(&data), black_box(&hash_crc))
                    .unwrap()
            })
        });

        // Benchmark XXHash32
        group.bench_function("xxhash32/hash", |b| {
            b.iter(|| IntegrityAlgorithm::XXHash32.hash(black_box(&data)))
        });

        let hash_xx = IntegrityAlgorithm::XXHash32.hash(&data);
        group.bench_function("xxhash32/verify", |b| {
            b.iter(|| {
                IntegrityAlgorithm::XXHash32
                    .verify(black_box(&data), black_box(&hash_xx))
                    .unwrap()
            })
        });

        group.finish();
    }
}

fn bench_integrity_incremental(c: &mut Criterion) {
    let sizes = [1024, 16384, 65536, 262144];
    let chunk_size = 4096;

    for &size in &sizes {
        let data = generate_test_data(size, DataPattern::Random);
        let group_name = format!("integrity/incremental/{}", size);
        let mut group = c.benchmark_group(group_name);
        group.throughput(Throughput::Bytes(size as u64));

        // Benchmark CRC32C incremental
        group.bench_function("crc32c", |b| {
            b.iter(|| {
                let mut hasher = IntegrityAlgorithm::Crc32c.hasher();
                for chunk in data.chunks(chunk_size) {
                    hasher.update(black_box(chunk));
                }
                hasher.finalize()
            })
        });

        // Benchmark XXHash32 incremental
        group.bench_function("xxhash32", |b| {
            b.iter(|| {
                let mut hasher = IntegrityAlgorithm::XXHash32.hasher();
                for chunk in data.chunks(chunk_size) {
                    hasher.update(black_box(chunk));
                }
                hasher.finalize()
            })
        });

        group.finish();
    }
}

// ============================================================================
// Combined Pipeline Benchmarks
// ============================================================================

fn bench_compress_encrypt_pipeline(c: &mut Criterion) {
    let sizes = [4096, 16384, 65536];

    for &size in &sizes {
        let data = generate_test_data(size, DataPattern::Repetitive);
        let group_name = format!("pipeline/compress_encrypt/{}", size);
        let mut group = c.benchmark_group(group_name);
        group.throughput(Throughput::Bytes(size as u64));

        // LZ4 + AES-256-GCM
        let key = EncryptionAlgorithm::Aes256Gcm.generate_key();
        let nonce = EncryptionAlgorithm::Aes256Gcm.generate_nonce();

        group.bench_function("lz4_aes256gcm", |b| {
            b.iter(|| {
                let compressed = CompressionAlgorithm::Lz4
                    .compress(black_box(&data))
                    .unwrap();
                EncryptionAlgorithm::Aes256Gcm
                    .encrypt(black_box(&key), black_box(&nonce), black_box(&compressed))
                    .unwrap()
            })
        });

        // Zstd + ChaCha20-Poly1305
        let key_chacha = EncryptionAlgorithm::ChaCha20Poly1305.generate_key();
        let nonce_chacha = EncryptionAlgorithm::ChaCha20Poly1305.generate_nonce();

        group.bench_function("zstd_chacha20poly1305", |b| {
            b.iter(|| {
                let compressed = CompressionAlgorithm::Zstd
                    .compress(black_box(&data))
                    .unwrap();
                EncryptionAlgorithm::ChaCha20Poly1305
                    .encrypt(
                        black_box(&key_chacha),
                        black_box(&nonce_chacha),
                        black_box(&compressed),
                    )
                    .unwrap()
            })
        });

        group.finish();
    }
}

fn bench_full_pipeline(c: &mut Criterion) {
    let sizes = [4096, 16384, 65536];

    for &size in &sizes {
        let data = generate_test_data(size, DataPattern::Repetitive);
        let group_name = format!("pipeline/full/{}", size);
        let mut group = c.benchmark_group(group_name);
        group.throughput(Throughput::Bytes(size as u64));

        let key = EncryptionAlgorithm::Aes256Gcm.generate_key();
        let nonce = EncryptionAlgorithm::Aes256Gcm.generate_nonce();

        // Compress -> Hash -> Encrypt
        group.bench_function("compress_hash_encrypt", |b| {
            b.iter(|| {
                let compressed = CompressionAlgorithm::Zstd
                    .compress(black_box(&data))
                    .unwrap();
                let _hash = IntegrityAlgorithm::Crc32c.hash(black_box(&compressed));
                EncryptionAlgorithm::Aes256Gcm
                    .encrypt(black_box(&key), black_box(&nonce), black_box(&compressed))
                    .unwrap()
            })
        });

        // Full roundtrip
        group.bench_function("full_roundtrip", |b| {
            b.iter(|| {
                // Compress
                let compressed = CompressionAlgorithm::Zstd
                    .compress(black_box(&data))
                    .unwrap();
                // Hash
                let hash = IntegrityAlgorithm::Crc32c.hash(&compressed);
                // Encrypt
                let encrypted = EncryptionAlgorithm::Aes256Gcm
                    .encrypt(&key, &nonce, &compressed)
                    .unwrap();
                // Decrypt
                let decrypted = EncryptionAlgorithm::Aes256Gcm
                    .decrypt(&key, &nonce, &encrypted)
                    .unwrap();
                // Verify
                IntegrityAlgorithm::Crc32c
                    .verify(&decrypted, &hash)
                    .unwrap();
                // Decompress
                CompressionAlgorithm::Zstd
                    .decompress(&decrypted, Some(size))
                    .unwrap()
            })
        });

        group.finish();
    }
}

// ============================================================================
// Compression Ratio Analysis
// ============================================================================

fn bench_compression_ratios(c: &mut Criterion) {
    let size = 65536;
    let patterns = [
        DataPattern::Zeros,
        DataPattern::Ones,
        DataPattern::Random,
        DataPattern::Repetitive,
        DataPattern::Sequential,
    ];

    let mut group = c.benchmark_group("compression/ratios");

    for &pattern in &patterns {
        let data = generate_test_data(size, pattern);

        for algo in [
            CompressionAlgorithm::Lz4,
            CompressionAlgorithm::Zstd,
            CompressionAlgorithm::Snappy,
        ] {
            let compressed = algo.compress(&data).unwrap();
            let ratio = (compressed.len() as f64 / data.len() as f64) * 100.0;

            group.bench_with_input(
                BenchmarkId::new(format!("{}/{}", algo.name(), pattern.name()), size),
                &data,
                |b, data| b.iter(|| algo.compress(black_box(data)).unwrap()),
            );

            // Print compression ratio (this will appear in benchmark output)
            println!(
                "{} on {} data: {:.2}% of original size",
                algo.name(),
                pattern.name(),
                ratio
            );
        }
    }

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    compression_benches,
    bench_compression_algorithms,
    bench_compression_into_buffer,
    bench_compression_ratios,
);

criterion_group!(
    encryption_benches,
    bench_encryption_algorithms,
    bench_encryption_key_generation,
    bench_encryption_nonce_generation,
);

criterion_group!(
    integrity_benches,
    bench_integrity_algorithms,
    bench_integrity_incremental,
);

criterion_group!(
    pipeline_benches,
    bench_compress_encrypt_pipeline,
    bench_full_pipeline,
);

criterion_main!(
    compression_benches,
    encryption_benches,
    integrity_benches,
    pipeline_benches,
);
