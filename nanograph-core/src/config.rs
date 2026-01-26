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

//! Nanograph Configuration
//!
mod network;
mod storage;

pub use self::network::NetworkConfig;
pub use self::storage::{StorageConfig, TablespaceConfig};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ServerConfig {
    pub network: NetworkConfig,
    pub storage: StorageConfig,
}

#[cfg(test)]
mod tests {
    use crate::config::storage::TablespaceConfig;
    use crate::config::{NetworkConfig, ServerConfig, StorageConfig};
    use crate::object::NodeId;
    use std::collections::HashMap;

    fn test_load_config() {
        let test_file = r#"
# Example Server Configuration
---
network:
  node: 0
  addr: 0.0.0.0:4000
storage:
  system_path: /tmp/nanograph/system
  log_path: /tmp/nanograph/logs
  tablespaces:
    hot_data:
      storage_path: /tmp/nanograph/data/hot
    warm_data:
      storage_path: /tmp/nanograph/data/warm
    cold_data:
      storage_path: /tmp/nanograph/data/cold
"#;
        let test_config: ServerConfig = serde_yml::from_slice(test_file.as_bytes()).unwrap();
        assert_eq!(
            test_config,
            ServerConfig {
                network: NetworkConfig {
                    node: NodeId::new(0),
                    addr: "0.0.0.0:4000".parse().unwrap()
                },
                storage: StorageConfig {
                    system_path: "/tmp/nanograph/system".to_string(),
                    log_path: "/tmp/nanograph/logs".to_string(),
                    tablespaces: HashMap::from_iter(vec![
                        (
                            "hot_data".to_string(),
                            TablespaceConfig {
                                storage_path: "/tmp/nanograph/data/hot".to_string()
                            }
                        ),
                        (
                            "warm_data".to_string(),
                            TablespaceConfig {
                                storage_path: "/tmp/nanograph/data/warm".to_string()
                            }
                        ),
                        (
                            "cold_data".to_string(),
                            TablespaceConfig {
                                storage_path: "/tmp/nanograph/data/cold".to_string()
                            }
                        ),
                    ]),
                }
            }
        )
    }
}
