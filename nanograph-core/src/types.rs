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

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Millisecond Timestamp for Multiversion Concurrency Control (MVCC)
#[derive(Copy, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct Timestamp(DateTime<Utc>);

impl Timestamp {
    /// Current Timestamp in Milliseconds
    pub fn now() -> Timestamp {
        Timestamp(Utc::now())
    }
    /// Epoch Timestamp in Milliseconds
    pub fn epoch() -> Timestamp {
        Timestamp(DateTime::<Utc>::UNIX_EPOCH)
    }
    /// Create a Timestamp from milliseconds since the Unix epoch
    pub fn from_millis(millis: i64) -> Timestamp {
        Timestamp(
            DateTime::<Utc>::from_timestamp_millis(millis).unwrap_or(DateTime::<Utc>::UNIX_EPOCH),
        )
    }
    /// Convert timestamp into milliseconds since the Unix epoch
    pub fn as_millis(&self) -> i64 {
        self.0.timestamp_millis()
    }
}

impl std::fmt::Display for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Timestamp({}ms)", self.0)
    }
}

impl Serialize for Timestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_i64(self.as_millis())
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Timestamp::from_millis(i64::deserialize(deserializer)?))
    }
}

/// Enumeration used for property updates
#[derive(Clone, Debug)]
pub enum PropertyUpdate {
    /// Set Option
    Set(String, String),
    /// Clear Option
    Clear(String),
}

impl PropertyUpdate {
    pub fn key(&self) -> &str {
        match self {
            PropertyUpdate::Set(key, _) => key,
            PropertyUpdate::Clear(key) => key,
        }
    }
}
