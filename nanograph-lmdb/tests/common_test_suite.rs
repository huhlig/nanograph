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

//! Test LMDB implementation using the common KeyValueShardStore test suite

use nanograph_kvt::test_suite::run_kvstore_test_suite;
use nanograph_lmdb::LMDBKeyValueStore;
use tempfile::TempDir;

#[tokio::test]
async fn test_lmdb_with_common_suite() {
    let temp_dir = TempDir::new().unwrap();
    let store = LMDBKeyValueStore::new().with_base_dir(temp_dir.path().to_path_buf());

    run_kvstore_test_suite(&store).await;
}

// Made with Bob
