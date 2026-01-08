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

use async_trait::async_trait;
use openraft::{Config, Raft, RaftStorage};
use nanograph_kvt::{KeyValueStore, ShardId};

struct ShardStateMachine {
    storage: Box<dyn KeyValueStore>,
    shard_id: ShardId,
}

#[async_trait]
impl RaftStorage for ShardStateMachine {
    type SnapshotData = ShardSnapshot;

    async fn apply_entry(&mut self, entry: &Entry) -> Result<Response> {
        match entry.payload {
            EntryPayload::Normal(ref data) => {
                let op: Operation = bincode::deserialize(data)?;
                self.storage.apply(op).await
            }
            _ => Ok(Response::default())
        }
    }

    async fn build_snapshot(&mut self) -> Result<Snapshot> {
        // Use existing SSTable snapshot mechanism
        self.storage.create_snapshot().await
    }
}