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

use nanograph_core::object::NodeId;
use std::time::Duration;

pub struct KeyValueDatabaseConfig {
    /// Cluster Node Id (Defaults to zero)
    pub node_id: NodeId,
    /// Cache Time to Live (Defaults to 1 hour)
    pub cache_ttl: Duration,
}

impl Default for KeyValueDatabaseConfig {
    fn default() -> Self {
        Self {
            node_id: NodeId::default(),
            cache_ttl: Duration::from_secs(60 * 60),
        }
    }
}
