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

//! # Nanograph Write Ahead Log Service
//!
//! This crate provides a high-performance Write-Ahead Log (WAL) implementation with
//! built-in metrics collection using the `metrics` crate.
//!
//! ## Features
//!
//! - **Durability Guarantees**: Memory, Flush, and Sync durability levels
//! - **Metrics Integration**: Comprehensive metrics for monitoring WAL operations
//! - **Configurable**: Support for compression, encryption, and integrity checking
//! - **Virtual File System**: Works with any VFS implementation
//!
//! ## Examples
//!
//! ### Write Example
//! ```rust
//! # use nanograph_wal::{WriteAheadLogManager, WriteAheadLogConfig, WriteAheadLogRecord, Durability};
//! # use nanograph_vfs::MemoryFileSystem;
//! # let fs = MemoryFileSystem::new();
//! # let path = "/wal";
//! # let config = WriteAheadLogConfig::new(0);
//! let wal = WriteAheadLogManager::new(fs, path, config)?;
//! let mut writer = wal.writer()?;
//! let record = WriteAheadLogRecord { kind: 1, payload: b"hello" };
//! let lsn = writer.append(record, Durability::Flush)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ### Read Example
//! ```rust
//! # use nanograph_wal::{WriteAheadLogManager, WriteAheadLogConfig, WriteAheadLogRecord, Durability, LogSequenceNumber};
//! # use nanograph_vfs::MemoryFileSystem;
//! # let fs = MemoryFileSystem::new();
//! # let path = "/wal";
//! # let config = WriteAheadLogConfig::new(0);
//! let wal = WriteAheadLogManager::new(fs, path, config)?;
//!
//! // Load snapshot at LSN S
//! # let mut writer = wal.writer()?;
//! # let record = WriteAheadLogRecord { kind: 1, payload: b"hello" };
//! # let snapshot_lsn = writer.append(record, Durability::Flush)?;
//!
//! // Replay WAL
//! let mut reader = wal.reader_from(snapshot_lsn)?;
//! while let Some(entry) = reader.next()? {
//!     println!("Record: kind={}, payload={:?}", entry.kind, entry.payload);
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Metrics
//!
//! The WAL automatically collects metrics for all operations. Available metrics include:
//!
//! - **Operations**: `nanograph_wal_operations_total` - Counter of operations by type and status
//! - **Bytes**: `nanograph_wal_bytes_written_total`, `nanograph_wal_bytes_read_total`
//! - **Latency**: `nanograph_wal_operation_duration_microseconds` - Operation duration histogram
//! - **State**: `nanograph_wal_active_segments`, `nanograph_wal_current_lsn`
//! - **Records**: `nanograph_wal_records_appended_total`, `nanograph_wal_records_read_total`
//!
//! See the [`metrics`] module for the complete list of available metrics.
//!
//! ## On-Disk Format
//! ### Segment Header Format
//!
//! | Field          | Type  | Size | Notes                 |
//! |----------------|-------|------|-----------------------|
//! | magic          | u32   | 4    | Magic Number          |
//! | version        | u16   | 2    | Segment Version       |
//! | shard_id       | u64   | 4    | Shard ID              |
//! | segment_id     | u64   | 4    | Segment ID            |
//! | start_offset   | u128  | 16   | Start Offset          |
//! | created_at     | u32   | 4    | Magic Number          |
//! | integrity      | u8    | 1    | Integrity Algorithm   |
//! | compression    | u8    | 1    | Compression Algorithm |
//! | encryption     | u8    | 1    | Encryption Algorithm  |
//! | encryption_key | u128  | 16   | Encryption Key ID     |
//! | checksum       | u32   | 4    | Checksum              |
//!
//! ### Record Format
//!
//! | Field    | Type | Size | Notes                 |
//! |----------|------|------|-----------------------|
//! | magic    | u32  | 4    | Magic Number          |
//! | version  | u16  | 2    | Record Layout Version |
//! | kind     | u16  | 2    | Record Kind           |
//! | len      | u32  | 4    | Payload Length        |
//! | payload  | [u8] | len  | Payload Data          |
//! | checksum | u32  | 4    | Checksum              |
//!
//!
//!
//!

#![deny(unsafe_code)]
#![warn(
    clippy::cargo,
    missing_docs,
    clippy::pedantic,
    future_incompatible,
    rust_2018_idioms
)]
#![allow(
    clippy::option_if_let_else,
    clippy::module_name_repetitions,
    clippy::missing_errors_doc
)]

mod config;
mod lsn;
mod manager;
pub mod metrics;
mod reader;
mod result;
mod walfile;
mod writer;

pub use self::config::{
    CompressionAlgorithm, Durability, EncryptionAlgorithm, IntegrityAlgorithm, WriteAheadLogConfig,
};
pub use self::lsn::LogSequenceNumber;
pub use self::manager::WriteAheadLogManager;
pub use self::reader::{WriteAheadLogEntry, WriteAheadLogReader};
pub use self::result::{WriteAheadLogError, WriteAheadLogResult};
pub use self::walfile::{HEADER_SIZE, WriteAheadLogFile, WriteAheadLogSegmentHeader};
pub use self::writer::{WriteAheadLogRecord, WriteAheadLogWriter};
