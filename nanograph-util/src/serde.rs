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

const CRC: crc::Crc<u32> = crc::Crc::<u32>::new(&crc::CRC_32_ISO_HDLC);

/// Serialize a value to a byte vector with a CRC32 checksum.
pub fn serialize<T: serde::Serialize>(value: &T) -> std::io::Result<Vec<u8>> {
    postcard::to_stdvec_crc32(value, CRC.digest())
        .map_err(|err: postcard::Error| std::io::Error::other(err))
}

/// Deserialize a value from a byte vector with a CRC32 checksum.
pub fn deserialize<'de, T: serde::de::DeserializeOwned>(bytes: &[u8]) -> std::io::Result<T> {
    postcard::from_bytes_crc32::<T>(bytes, CRC.digest())
        .map_err(|err: postcard::Error| std::io::Error::other(err))
}
