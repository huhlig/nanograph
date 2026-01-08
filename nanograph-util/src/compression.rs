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

use crate::error::{Error, Result};
use std::io::Write;

/// Compression Algorithm
///
/// This enum defines the supported compression algorithms for the Nanograph system.
///
/// # Examples
///
/// ```
/// use nanograph_util::CompressionAlgorithm;
///
/// let data = b"hello world";
/// let algorithm = CompressionAlgorithm::Lz4;
/// let compressed = algorithm.compress(data).unwrap();
/// let decompressed = algorithm.decompress(&compressed, Some(data.len())).unwrap();
///
/// assert_eq!(data, decompressed.as_slice());
/// ```
#[derive(Copy, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[repr(u8)]
pub enum CompressionAlgorithm {
    /// No compression
    #[default]
    None = 0,
    /// LZ4 compression - fast compression with decent ratio
    Lz4 = 1,
    /// Zstd compression - excellent compression ratio with good speed
    Zstd = 2,
    /// Snappy compression - very fast compression/decompression
    Snappy = 3,
}

impl CompressionAlgorithm {
    /// Convert the compression algorithm to a u8 value for serialization
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::CompressionAlgorithm;
    ///
    /// assert_eq!(CompressionAlgorithm::None.as_u8(), 0);
    /// assert_eq!(CompressionAlgorithm::Lz4.as_u8(), 1);
    /// ```
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Convert a u8 value to a compression algorithm, returning None if invalid
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::CompressionAlgorithm;
    ///
    /// assert_eq!(CompressionAlgorithm::from_u8(1), Some(CompressionAlgorithm::Lz4));
    /// assert_eq!(CompressionAlgorithm::from_u8(255), None);
    /// ```
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(CompressionAlgorithm::None),
            1 => Some(CompressionAlgorithm::Lz4),
            2 => Some(CompressionAlgorithm::Zstd),
            3 => Some(CompressionAlgorithm::Snappy),
            _ => None,
        }
    }

    /// Get the name of the compression algorithm
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::CompressionAlgorithm;
    ///
    /// assert_eq!(CompressionAlgorithm::Lz4.name(), "lz4");
    /// ```
    pub const fn name(self) -> &'static str {
        match self {
            CompressionAlgorithm::None => "none",
            CompressionAlgorithm::Lz4 => "lz4",
            CompressionAlgorithm::Zstd => "zstd",
            CompressionAlgorithm::Snappy => "snappy",
        }
    }

    /// Get the maximum compressed size for the given input size
    /// This is useful for pre-allocating output buffers
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::CompressionAlgorithm;
    ///
    /// let input_size = 1024;
    /// let max_size = CompressionAlgorithm::Lz4.max_compressed_size(input_size);
    /// assert!(max_size >= input_size);
    /// ```
    pub fn max_compressed_size(self, input_size: usize) -> usize {
        match self {
            CompressionAlgorithm::None => input_size,
            CompressionAlgorithm::Lz4 => {
                lz4::block::compress_bound(input_size).unwrap_or(input_size + 1024)
            }
            CompressionAlgorithm::Zstd => zstd::zstd_safe::compress_bound(input_size),
            CompressionAlgorithm::Snappy => snap::raw::max_compress_len(input_size),
        }
    }

    /// Compress data using this algorithm
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::CompressionAlgorithm;
    ///
    /// let algorithm = CompressionAlgorithm::Lz4;
    /// let data = b"some data to compress";
    /// let compressed = algorithm.compress(data).unwrap();
    /// ```
    pub fn compress(self, input: &[u8]) -> Result<Vec<u8>> {
        match self {
            CompressionAlgorithm::None => Ok(input.to_vec()),
            CompressionAlgorithm::Lz4 => lz4::block::compress(input, None, false)
                .map_err(|e| Error::Compression(format!("LZ4 compression failed: {}", e))),
            CompressionAlgorithm::Zstd => zstd::encode_all(input, 3)
                .map_err(|e| Error::Compression(format!("Zstd compression failed: {}", e))),
            CompressionAlgorithm::Snappy => {
                let mut encoder = snap::raw::Encoder::new();
                encoder
                    .compress_vec(input)
                    .map_err(|e| Error::Compression(format!("Snappy compression failed: {}", e)))
            }
        }
    }

    /// Compress data into a pre-allocated buffer
    /// Returns the number of bytes written
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::CompressionAlgorithm;
    ///
    /// let algorithm = CompressionAlgorithm::Lz4;
    /// let data = b"some data to compress";
    /// let mut output = vec![0u8; algorithm.max_compressed_size(data.len())];
    /// let compressed_size = algorithm.compress_into(data, &mut output).unwrap();
    /// ```
    pub fn compress_into(self, input: &[u8], output: &mut [u8]) -> Result<usize> {
        match self {
            CompressionAlgorithm::None => {
                if output.len() < input.len() {
                    return Err(Error::BufferTooSmall {
                        required: input.len(),
                        available: output.len(),
                    });
                }
                output[..input.len()].copy_from_slice(input);
                Ok(input.len())
            }
            CompressionAlgorithm::Lz4 => lz4::block::compress_to_buffer(input, None, false, output)
                .map_err(|e| Error::Compression(format!("LZ4 compression failed: {}", e))),
            CompressionAlgorithm::Zstd => {
                let mut cursor = std::io::Cursor::new(output);
                let mut encoder = zstd::Encoder::new(&mut cursor, 3).map_err(|e| {
                    Error::Compression(format!("Zstd encoder creation failed: {}", e))
                })?;
                encoder
                    .write_all(input)
                    .map_err(|e| Error::Compression(format!("Zstd compression failed: {}", e)))?;
                encoder
                    .finish()
                    .map_err(|e| Error::Compression(format!("Zstd finish failed: {}", e)))?;
                Ok(cursor.position() as usize)
            }
            CompressionAlgorithm::Snappy => {
                let mut encoder = snap::raw::Encoder::new();
                encoder
                    .compress(input, output)
                    .map_err(|e| Error::Compression(format!("Snappy compression failed: {}", e)))
            }
        }
    }

    /// Decompress data using this algorithm
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::CompressionAlgorithm;
    ///
    /// let algorithm = CompressionAlgorithm::Lz4;
    /// let data = b"some data to compress";
    /// let compressed = algorithm.compress(data).unwrap();
    /// let decompressed = algorithm.decompress(&compressed, Some(data.len())).unwrap();
    /// assert_eq!(data, decompressed.as_slice());
    /// ```
    pub fn decompress(self, input: &[u8], expected_size: Option<usize>) -> Result<Vec<u8>> {
        match self {
            CompressionAlgorithm::None => Ok(input.to_vec()),
            CompressionAlgorithm::Lz4 => {
                let size = expected_size.unwrap_or(input.len() * 4);
                lz4::block::decompress(input, Some(size as i32))
                    .map_err(|e| Error::Decompression(format!("LZ4 decompression failed: {}", e)))
            }
            CompressionAlgorithm::Zstd => zstd::decode_all(input)
                .map_err(|e| Error::Decompression(format!("Zstd decompression failed: {}", e))),
            CompressionAlgorithm::Snappy => {
                let mut decoder = snap::raw::Decoder::new();
                decoder.decompress_vec(input).map_err(|e| {
                    Error::Decompression(format!("Snappy decompression failed: {}", e))
                })
            }
        }
    }

    /// Decompress data into a pre-allocated buffer
    /// Returns the number of bytes written
    ///
    /// # Examples
    ///
    /// ```
    /// use nanograph_util::CompressionAlgorithm;
    ///
    /// let algorithm = CompressionAlgorithm::Lz4;
    /// let data = b"some data to compress";
    /// let compressed = algorithm.compress(data).unwrap();
    /// let mut output = vec![0u8; data.len()];
    /// let decompressed_size = algorithm.decompress_into(&compressed, &mut output).unwrap();
    /// assert_eq!(data.len(), decompressed_size);
    /// ```
    pub fn decompress_into(self, input: &[u8], output: &mut [u8]) -> Result<usize> {
        match self {
            CompressionAlgorithm::None => {
                if output.len() < input.len() {
                    return Err(Error::BufferTooSmall {
                        required: input.len(),
                        available: output.len(),
                    });
                }
                output[..input.len()].copy_from_slice(input);
                Ok(input.len())
            }
            CompressionAlgorithm::Lz4 => {
                lz4::block::decompress_to_buffer(input, Some(output.len() as i32), output)
                    .map_err(|e| Error::Decompression(format!("LZ4 decompression failed: {}", e)))
            }
            CompressionAlgorithm::Zstd => {
                let size = zstd::bulk::decompress_to_buffer(input, output).map_err(|e| {
                    Error::Decompression(format!("Zstd decompression failed: {}", e))
                })?;
                Ok(size)
            }
            CompressionAlgorithm::Snappy => {
                let mut decoder = snap::raw::Decoder::new();
                decoder.decompress(input, output).map_err(|e| {
                    Error::Decompression(format!("Snappy decompression failed: {}", e))
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_DATA: &[u8] = b"Hello, World! This is a test string for compression. It should compress well because it has repetitive patterns. Hello, World! Hello, World!";

    #[test]
    fn test_none_compression() {
        let compressed = CompressionAlgorithm::None.compress(TEST_DATA).unwrap();
        assert_eq!(compressed, TEST_DATA);
        let decompressed = CompressionAlgorithm::None
            .decompress(&compressed, None)
            .unwrap();
        assert_eq!(decompressed, TEST_DATA);
    }

    #[test]
    fn test_lz4_compression() {
        let compressed = CompressionAlgorithm::Lz4.compress(TEST_DATA).unwrap();
        assert!(compressed.len() < TEST_DATA.len());
        let decompressed = CompressionAlgorithm::Lz4
            .decompress(&compressed, Some(TEST_DATA.len()))
            .unwrap();
        assert_eq!(decompressed, TEST_DATA);
    }

    #[test]
    fn test_zstd_compression() {
        let compressed = CompressionAlgorithm::Zstd.compress(TEST_DATA).unwrap();
        assert!(compressed.len() < TEST_DATA.len());
        let decompressed = CompressionAlgorithm::Zstd
            .decompress(&compressed, None)
            .unwrap();
        assert_eq!(decompressed, TEST_DATA);
    }

    #[test]
    fn test_snappy_compression() {
        let compressed = CompressionAlgorithm::Snappy.compress(TEST_DATA).unwrap();
        let decompressed = CompressionAlgorithm::Snappy
            .decompress(&compressed, None)
            .unwrap();
        assert_eq!(decompressed, TEST_DATA);
    }

    #[test]
    fn test_compress_into() {
        for algo in [
            CompressionAlgorithm::None,
            CompressionAlgorithm::Lz4,
            CompressionAlgorithm::Zstd,
            CompressionAlgorithm::Snappy,
        ] {
            let max_size = algo.max_compressed_size(TEST_DATA.len());
            let mut buffer = vec![0u8; max_size];
            let size = algo.compress_into(TEST_DATA, &mut buffer).unwrap();

            let mut output = vec![0u8; TEST_DATA.len()];
            let decompressed_size = algo.decompress_into(&buffer[..size], &mut output).unwrap();
            assert_eq!(&output[..decompressed_size], TEST_DATA);
        }
    }

    #[test]
    fn test_algorithm_serialization() {
        for algo in [
            CompressionAlgorithm::None,
            CompressionAlgorithm::Lz4,
            CompressionAlgorithm::Zstd,
            CompressionAlgorithm::Snappy,
        ] {
            let byte = algo.as_u8();
            let restored = CompressionAlgorithm::from_u8(byte).unwrap();
            assert_eq!(algo, restored);
        }
    }
}

// Made with Bob
