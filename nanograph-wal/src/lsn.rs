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

use crate::walfile::HEADER_SIZE;

/// Log Sequence Number (LSN)
/// Represents a unique position within the Write Ahead Log.
///
/// # Examples
///
/// ```rust
/// use nanograph_wal::LogSequenceNumber;
///
/// let lsn1 = LogSequenceNumber { segment_id: 1, offset: 100 };
/// let lsn2 = LogSequenceNumber { segment_id: 1, offset: 200 };
/// assert!(lsn1 < lsn2);
/// ```
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct LogSequenceNumber {
    /// Identifier of the WAL segment file
    pub segment_id: u64,
    /// Byte offset within the segment file
    pub offset: u64,
}

impl LogSequenceNumber {
    /// Initial LSN - points to the first record position (after the segment header)
    pub const ZERO: LogSequenceNumber = LogSequenceNumber {
        segment_id: 0,
        offset: HEADER_SIZE as u64,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lsn_ordering() {
        let lsn1 = LogSequenceNumber {
            segment_id: 1,
            offset: 100,
        };
        let lsn2 = LogSequenceNumber {
            segment_id: 1,
            offset: 200,
        };
        let lsn3 = LogSequenceNumber {
            segment_id: 2,
            offset: 50,
        };

        assert!(lsn1 < lsn2);
        assert!(lsn2 < lsn3);
        assert!(lsn1 < lsn3);
    }

    #[test]
    fn test_lsn_default_and_zero() {
        // Note: Default is {0, 0} but ZERO is {0, HEADER_SIZE}
        assert_eq!(LogSequenceNumber::ZERO.segment_id, 0);
        assert_eq!(LogSequenceNumber::ZERO.offset, HEADER_SIZE as u64);
    }
}
