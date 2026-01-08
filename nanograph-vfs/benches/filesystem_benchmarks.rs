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
use nanograph_vfs::{File, FileSystem, MemoryFileSystem};
use std::hint::black_box;
use std::io::{Read, Seek, SeekFrom, Write};

fn bench_file_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_creation");

    for size in [1, 10, 100, 1000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let fs = MemoryFileSystem::new();
                for i in 0..size {
                    let path = format!("/file_{}.txt", i);
                    fs.create_file(&path).unwrap();
                }
            });
        });
    }

    group.finish();
}

fn bench_file_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_write");

    for size in [1024, 4096, 16384, 65536, 262144].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let fs = MemoryFileSystem::new();
            let data = vec![0u8; size];

            b.iter(|| {
                let mut file = fs.create_file("/bench.bin").unwrap();
                file.write_all(black_box(&data)).unwrap();
                fs.remove_file("/bench.bin").unwrap();
            });
        });
    }

    group.finish();
}

fn bench_file_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_read");

    for size in [1024, 4096, 16384, 65536, 262144].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let fs = MemoryFileSystem::new();
            let data = vec![42u8; size];

            // Setup: create file with data
            {
                let mut file = fs.create_file("/bench.bin").unwrap();
                file.write_all(&data).unwrap();
            }

            b.iter(|| {
                let mut file = fs.open_file("/bench.bin").unwrap();
                let mut buffer = vec![0u8; size];
                file.read_exact(black_box(&mut buffer)).unwrap();
                black_box(buffer);
            });
        });
    }

    group.finish();
}

fn bench_file_seek(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_seek");

    let fs = MemoryFileSystem::new();
    let size = 1024 * 1024; // 1MB file
    let data = vec![0u8; size];

    {
        let mut file = fs.create_file("/bench.bin").unwrap();
        file.write_all(&data).unwrap();
    }

    group.bench_function("seek_start", |b| {
        b.iter(|| {
            let mut file = fs.open_file("/bench.bin").unwrap();
            for i in 0..100 {
                file.seek(SeekFrom::Start(black_box(i * 1024))).unwrap();
            }
        });
    });

    group.bench_function("seek_end", |b| {
        b.iter(|| {
            let mut file = fs.open_file("/bench.bin").unwrap();
            for i in 0..100 {
                file.seek(SeekFrom::End(black_box(-(i * 1024) as i64)))
                    .unwrap();
            }
        });
    });

    group.bench_function("seek_current", |b| {
        b.iter(|| {
            let mut file = fs.open_file("/bench.bin").unwrap();
            for i in 0..100 {
                file.seek(SeekFrom::Current(black_box(i * 10))).unwrap();
            }
        });
    });

    group.finish();
}

fn bench_offset_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("offset_operations");

    for size in [1024, 4096, 16384].iter() {
        group.throughput(Throughput::Bytes(*size as u64));

        let fs = MemoryFileSystem::new();
        let data = vec![42u8; *size];

        group.bench_with_input(
            BenchmarkId::new("write_at_offset", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let mut file = fs.create_file("/bench.bin").unwrap();
                    for i in 0..10 {
                        file.write_to_offset(black_box(i * size as u64), black_box(&data))
                            .unwrap();
                    }
                    fs.remove_file("/bench.bin").unwrap();
                });
            },
        );

        // Setup for read benchmark
        {
            let mut file = fs.create_file("/bench_read.bin").unwrap();
            for i in 0..10 {
                file.write_to_offset(i * (*size as u64), &data).unwrap();
            }
        }

        group.bench_with_input(
            BenchmarkId::new("read_at_offset", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let mut file = fs.open_file("/bench_read.bin").unwrap();
                    let mut buffer = vec![0u8; size];
                    for i in 0..10 {
                        file.read_at_offset(black_box(i * size as u64), black_box(&mut buffer))
                            .unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_directory_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("directory_operations");

    group.bench_function("create_directory", |b| {
        b.iter(|| {
            let fs = MemoryFileSystem::new();
            for i in 0..100 {
                let path = format!("/dir_{}", i);
                fs.create_directory(&path).unwrap();
            }
        });
    });

    group.bench_function("create_directory_all", |b| {
        b.iter(|| {
            let fs = MemoryFileSystem::new();
            for i in 0..100 {
                let path = format!("/a/b/c/d/e/dir_{}", i);
                fs.create_directory_all(&path).unwrap();
            }
        });
    });

    group.bench_function("list_directory", |b| {
        let fs = MemoryFileSystem::new();
        fs.create_directory("/test").unwrap();
        for i in 0..100 {
            fs.create_file(&format!("/test/file_{}.txt", i)).unwrap();
        }

        b.iter(|| {
            let entries = fs.list_directory(black_box("/test")).unwrap();
            black_box(entries);
        });
    });

    group.bench_function("remove_directory_all", |b| {
        b.iter_batched(
            || {
                let fs = MemoryFileSystem::new();
                fs.create_directory_all("/a/b/c/d/e").unwrap();
                for i in 0..50 {
                    fs.create_file(&format!("/a/b/c/file_{}.txt", i)).unwrap();
                }
                fs
            },
            |fs| {
                fs.remove_directory_all("/a").unwrap();
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn bench_concurrent_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_access");

    group.bench_function("parallel_file_creation", |b| {
        b.iter(|| {
            let fs = MemoryFileSystem::new();
            let handles: Vec<_> = (0..10)
                .map(|i| {
                    let fs_clone = fs.clone();
                    std::thread::spawn(move || {
                        for j in 0..10 {
                            let path = format!("/file_{}_{}.txt", i, j);
                            fs_clone.create_file(&path).unwrap();
                        }
                    })
                })
                .collect();

            for handle in handles {
                handle.join().unwrap();
            }
        });
    });

    group.bench_function("parallel_file_read_write", |b| {
        let fs = MemoryFileSystem::new();

        // Setup: create files
        for i in 0..10 {
            let mut file = fs.create_file(&format!("/file_{}.txt", i)).unwrap();
            file.write_all(b"initial content").unwrap();
        }

        b.iter(|| {
            let handles: Vec<_> = (0..10)
                .map(|i| {
                    let fs_clone = fs.clone();
                    std::thread::spawn(move || {
                        let mut file = fs_clone.open_file(&format!("/file_{}.txt", i)).unwrap();
                        let mut content = Vec::new();
                        file.read_to_end(&mut content).unwrap();
                        black_box(content);
                    })
                })
                .collect();

            for handle in handles {
                handle.join().unwrap();
            }
        });
    });

    group.finish();
}

fn bench_file_metadata(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_metadata");

    let fs = MemoryFileSystem::new();
    fs.create_file("/test.txt").unwrap();

    group.bench_function("exists", |b| {
        b.iter(|| {
            fs.exists(black_box("/test.txt")).unwrap();
        });
    });

    group.bench_function("is_file", |b| {
        b.iter(|| {
            fs.is_file(black_box("/test.txt")).unwrap();
        });
    });

    group.bench_function("is_directory", |b| {
        b.iter(|| {
            fs.is_directory(black_box("/test.txt")).unwrap();
        });
    });

    group.bench_function("filesize", |b| {
        b.iter(|| {
            fs.filesize(black_box("/test.txt")).unwrap();
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_file_creation,
    bench_file_write,
    bench_file_read,
    bench_file_seek,
    bench_offset_operations,
    bench_directory_operations,
    bench_concurrent_access,
    bench_file_metadata
);

criterion_main!(benches);

// Made with Bob
