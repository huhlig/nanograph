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

use nanograph_core::object::{Permission, PermissionGrant, ResourceScope, SecurityPrincipal, SubjectId, SystemUserRecord, UserId};
use nanograph_core::types::Timestamp;
use std::collections::HashMap;

pub fn create_test_principal() -> SecurityPrincipal {
    let user_record = SystemUserRecord {
        user_id: UserId::new(SubjectId::new(1)),
        username: "admin".to_string(),
        version: 1,
        created_at: Timestamp::now(),
        last_modified: Timestamp::now(),
        group_ids: vec![],
        role_ids: vec![],
        grants: vec![PermissionGrant::new(
            Permission::GlobalSuperuser,
            ResourceScope::System,
        )],
        enabled: true,
        password_hash: None,
        options: HashMap::new(),
        metadata: HashMap::new(),
    };
    SecurityPrincipal::from_system_user(&user_record, &[], &[])
}
