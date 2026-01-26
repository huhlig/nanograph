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

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Base path for system metadata on this node.
    pub system_path: String,
    /// Base path for system logs on this node.
    pub log_path: String,
    /// Tablespaces configured on this node.
    pub tablespaces: HashMap<String, TablespaceConfig>,
}

/// Configuration for a tablespace on this node, taken from configuration manager.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TablespaceConfig {
    /// Base path for this tablespace on this node
    pub storage_path: String,
}
