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

use crate::types::ConsensusTypeConfig;
use nanograph_core::object::NodeId;
use nanograph_wal::{Durability, LogSequenceNumber, WriteAheadLogManager};
use openraft::entry::RaftEntry;
use openraft::storage::{IOFlushed, RaftLogStorage};
use openraft::type_config::alias::{LogIdOf, VoteOf};
use openraft::{Entry, LogState, OptionalSend, RaftLogReader};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::ops::RangeBounds;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Shared inner state for both the log store and readers
/// Uses separate locks for each field to avoid deadlocks
struct ConsensusLogStoreInner {
    /// Last Purged Log ID
    last_purged_log_id: RwLock<Option<LogIdOf<ConsensusTypeConfig>>>,

    /// Persistent Log Storage
    log_store: Arc<WriteAheadLogManager>,

    /// Index of log indices to LSNs - maps log index to (segment_id, offset, payload)
    /// We cache the payload to avoid reading from WAL during concurrent operations
    index: RwLock<BTreeMap<u64, (LogSequenceNumber, Vec<u8>)>>,

    /// The current granted vote.
    vote: RwLock<Option<VoteOf<ConsensusTypeConfig>>>,

    /// The last committed log id.
    committed: RwLock<Option<LogIdOf<ConsensusTypeConfig>>>,
}

/// The main log store that implements RaftLogStorage
#[derive(Clone)]
pub struct ConsensusLogStore {
    inner: Arc<ConsensusLogStoreInner>,
}

impl ConsensusLogStore {
    pub fn new(log_store: Arc<WriteAheadLogManager>) -> Self {
        Self {
            inner: Arc::new(ConsensusLogStoreInner {
                last_purged_log_id: RwLock::new(None),
                log_store,
                index: RwLock::new(BTreeMap::new()),
                vote: RwLock::new(None),
                committed: RwLock::new(None),
            }),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(bound = "")]
enum ConsensusLogRecord {
    Committed(LogIdOf<ConsensusTypeConfig>),
    Entry(Entry<ConsensusTypeConfig>),
    Vote(VoteOf<ConsensusTypeConfig>),
}

impl RaftLogStorage<ConsensusTypeConfig> for ConsensusLogStore {
    /// Log reader type - just use the store itself since it's Clone
    type LogReader = ConsensusLogStore;

    /// Returns the last deleted log id and the last log id.
    async fn get_log_state(&mut self) -> Result<LogState<ConsensusTypeConfig>, std::io::Error> {
        // IMPORTANT: Always acquire locks in the same order: index THEN last_purged_log_id
        // This matches the order in purge() to prevent deadlocks
        let index = self.inner.index.read().await;
        let last_purged_log_id = self.inner.last_purged_log_id.read().await.clone();

        let last_log_id = if let Some((_index, (_lsn, payload))) = index.last_key_value() {
            let record: ConsensusLogRecord = nanograph_util::deserialize(payload)?;
            if let ConsensusLogRecord::Entry(e) = record {
                Some(e.log_id.clone())
            } else {
                last_purged_log_id.clone()
            }
        } else {
            last_purged_log_id.clone()
        };

        Ok(LogState {
            last_purged_log_id,
            last_log_id,
        })
    }

    /// Get the log reader - just clone self
    async fn get_log_reader(&mut self) -> Self::LogReader {
        self.clone()
    }

    /// Save vote to storage.
    async fn save_vote(
        &mut self,
        vote: &VoteOf<ConsensusTypeConfig>,
    ) -> Result<(), std::io::Error> {
        let record = ConsensusLogRecord::Vote(vote.clone());
        let payload = nanograph_util::serialize(&record)?;

        // Write to WAL first, without holding across await
        {
            let mut writer = self
                .inner
                .log_store
                .writer()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            writer
                .append(
                    nanograph_wal::WriteAheadLogRecord {
                        kind: 1, // Vote
                        payload: &payload,
                    },
                    Durability::Sync,
                )
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        } // Writer dropped here, releasing any internal locks

        // Now update in-memory state
        *self.inner.vote.write().await = Some(vote.clone());
        Ok(())
    }

    /// Saves the last committed log id to storage.
    async fn save_committed(
        &mut self,
        committed: Option<LogIdOf<ConsensusTypeConfig>>,
    ) -> Result<(), std::io::Error> {
        // Write to WAL first, without holding across await
        if let Some(ref log_id) = committed {
            let record = ConsensusLogRecord::Committed(log_id.clone());
            let payload = nanograph_util::serialize(&record)?;
            {
                let mut writer = self
                    .inner
                    .log_store
                    .writer()
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                writer
                    .append(
                        nanograph_wal::WriteAheadLogRecord {
                            kind: 2, // Committed
                            payload: &payload,
                        },
                        Durability::Sync,
                    )
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            } // Writer dropped here, releasing any internal locks
        }

        // Now update in-memory state
        *self.inner.committed.write().await = committed;
        Ok(())
    }

    /// Return the last saved committed log id by [`Self::save_committed`].
    async fn read_committed(
        &mut self,
    ) -> Result<Option<LogIdOf<ConsensusTypeConfig>>, std::io::Error> {
        Ok(self.inner.committed.read().await.clone())
    }

    /// Append log entries and call the `callback` once logs are persisted on disk.
    async fn append<I>(
        &mut self,
        entries: I,
        callback: IOFlushed<ConsensusTypeConfig>,
    ) -> Result<(), std::io::Error>
    where
        I: IntoIterator<Item = Entry<ConsensusTypeConfig>> + OptionalSend,
        I::IntoIter: OptionalSend,
    {
        // Collect entries and serialize them first
        let mut entries_data = Vec::new();
        for entry in entries {
            let log_id = entry.log_id().clone();
            let record = ConsensusLogRecord::Entry(entry);
            let payload = nanograph_util::serialize(&record)?;
            entries_data.push((log_id, payload));
        }

        // Write to WAL first, collecting LSNs
        let mut lsns = Vec::new();
        {
            let mut writer = self
                .inner
                .log_store
                .writer()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

            for (_log_id, payload) in &entries_data {
                let lsn = writer
                    .append(
                        nanograph_wal::WriteAheadLogRecord {
                            kind: 0, // Entry
                            payload,
                        },
                        Durability::Buffered,
                    )
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                lsns.push(lsn);
            }
            writer
                .sync()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        } // Writer dropped here before acquiring index lock

        // Now update index after writer is dropped, caching payloads
        let mut index = self.inner.index.write().await;
        for ((log_id, payload), lsn) in entries_data.iter().zip(lsns.iter()) {
            index.insert(log_id.index, (*lsn, payload.clone()));
        }
        drop(index); // Explicitly drop index lock

        callback.io_completed(Ok(()));
        Ok(())
    }

    /// Truncate logs after `last_log_id`, exclusive
    async fn truncate_after(
        &mut self,
        last_log_id: Option<LogIdOf<ConsensusTypeConfig>>,
    ) -> Result<(), std::io::Error> {
        let mut index = self.inner.index.write().await;
        if let Some(last_log_id) = last_log_id {
            index.split_off(&(last_log_id.index + 1));
        } else {
            index.clear();
        }
        // WAL truncation is usually prefix-based (truncate_before).
        // For Raft truncate_after (suffix-based), we just remove from our index.
        // The WAL segments will eventually be rotated and purged.
        Ok(())
    }

    /// Purge logs up to `log_id`, inclusive
    async fn purge(&mut self, log_id: LogIdOf<ConsensusTypeConfig>) -> Result<(), std::io::Error> {
        // First, get the LSN to truncate before by reading the index
        let lsn_to_truncate = {
            let mut index = self.inner.index.write().await;

            let mut split_at = index.split_off(&(log_id.index + 1));
            std::mem::swap(&mut *index, &mut split_at);
            // split_at now contains entries <= log_id.index

            // Get the LSN of the first remaining entry
            index.first_key_value().map(|(_idx, (lsn, _payload))| *lsn)
        }; // index lock released here

        // Update last_purged_log_id after releasing index lock
        {
            let mut last_purged_log_id = self.inner.last_purged_log_id.write().await;
            *last_purged_log_id = Some(log_id.clone());
        } // last_purged_log_id lock released here

        // Truncate WAL if we have an LSN
        if let Some(lsn) = lsn_to_truncate {
            self.inner
                .log_store
                .truncate_before(lsn)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        }

        Ok(())
    }
}

/// Implement RaftLogReader for ConsensusLogStore so it can be used as its own reader
impl RaftLogReader<ConsensusTypeConfig> for ConsensusLogStore {
    /// Get a series of log entries from storage.
    async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Debug + OptionalSend>(
        &mut self,
        range: RB,
    ) -> Result<Vec<Entry<ConsensusTypeConfig>>, std::io::Error> {
        // Get cached payloads while holding the lock
        let payloads: Vec<Vec<u8>> = {
            let index = self.inner.index.read().await;
            index
                .range(range)
                .map(|(_, (_lsn, payload))| payload.clone())
                .collect()
        };

        let mut entries = Vec::new();
        for payload in payloads {
            let record: ConsensusLogRecord = nanograph_util::deserialize(&payload)?;
            if let ConsensusLogRecord::Entry(e) = record {
                entries.push(e);
            }
        }
        Ok(entries)
    }

    /// Return the last saved vote by [`RaftLogStorage::save_vote`].
    async fn read_vote(&mut self) -> Result<Option<VoteOf<ConsensusTypeConfig>>, std::io::Error> {
        let vote = self.inner.vote.read().await.clone();
        // If no vote has been saved, return a default vote with term 0
        Ok(Some(
            vote.unwrap_or_else(|| openraft::Vote::new(0, NodeId::new(0))),
        ))
    }
}

// Keep the old ConsensusLogReader type alias for backwards compatibility
// Type alias removed - was unused
