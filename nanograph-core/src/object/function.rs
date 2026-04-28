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

use crate::object::ObjectId;
use crate::types::{PropertyUpdate, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Function identifier
///
/// Uses u64 for globally unique identification within a schema.
/// Names are stored separately in metadata and mapped to IDs.
#[derive(
    Copy, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize,
)]
pub struct FunctionId(pub ObjectId);

impl FunctionId {
    /// Create a new table identifier.
    pub fn new(id: ObjectId) -> Self {
        Self(id)
    }

    /// Return the object identifier as an ObjectId.
    pub fn object(&self) -> ObjectId {
        self.0
    }
}

impl From<u32> for FunctionId {
    fn from(id: u32) -> Self {
        Self(ObjectId(id))
    }
}

impl std::fmt::Display for FunctionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Function({})", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_id() {
        let id = FunctionId::new(ObjectId::new(0x12345678));
        assert_eq!(id.object().as_u32(), 0x12345678);
        assert_eq!(FunctionId::from(0x12345678), id);
        assert_eq!(format!("{}", id), "Function(12345678)");
    }
}

/// Configuration for Function creation
#[derive(Clone, Debug)]
pub struct FunctionCreate {
    /// Name of the Function
    pub name: String,
    /// Body of the Function
    pub body: String,
    /// Additional engine-specific options
    pub options: HashMap<String, String>,
    /// Function Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

impl FunctionCreate {
    /// Create a new Function creation configuration.
    ///
    /// # Arguments
    ///
    /// * `name`: The name of the new Function.
    /// * `body`: The body of the new Function.
    pub fn new(name: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            body: body.into(),
            options: HashMap::new(),
            metadata: HashMap::new(),
        }
    }

    /// Add or update a configuration option for the Function.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the option to set.
    /// * `value`: The value to assign to the option.
    pub fn with_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.options.insert(key.into(), value.into());
        self
    }
}

/// Configuration for Function update
#[derive(Clone, Debug, Default)]
pub struct FunctionUpdate {
    /// New name for the Function
    pub name: Option<String>,
    /// New body for the Function
    pub body: Option<String>,
    /// Function configuration options to update
    pub options: Vec<PropertyUpdate>,
    /// Function metadata to update
    pub metadata: Vec<PropertyUpdate>,
}

impl FunctionUpdate {
    /// Set a new name for the Function.
    ///
    /// # Arguments
    ///
    /// * `name`: The new name to set.
    pub fn set_name(&mut self, name: impl Into<String>) -> &mut Self {
        self.name = Some(name.into());
        self
    }
    /// Set a new body for the Function.
    ///
    /// # Arguments
    ///
    /// * `body`: The new body to set.
    pub fn set_body(&mut self, body: impl Into<String>) -> &mut Self {
        self.body = Some(body.into());
        self
    }

    /// Add or update a configuration option for the Function.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the option to set.
    /// * `value`: The value to assign to the option.
    pub fn set_option(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.options
            .push(PropertyUpdate::Set(key.into(), value.into()));
        self
    }
    /// Clear a configuration option from the Function.
    ///
    /// # Arguments
    ///
    /// * `key`: The key of the option to clear.
    pub fn clear_option(&mut self, key: impl Into<String>) -> &mut Self {
        self.options.push(PropertyUpdate::Clear(key.into()));
        self
    }
}

/// Metadata for a function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionMetadata {
    /// Unique identifier for the function
    pub id: FunctionId,
    /// Name of the function
    pub name: String,
    /// Path of the function within the namespace hierarchy
    pub path: String,
    /// Timestamp when the function was created
    pub created_at: Timestamp,
    /// Timestamp when the function was last modified
    pub last_modified: Timestamp,
    /// Additional engine-specific options
    pub options: HashMap<String, String>,
    /// Function Metadata (Informative)
    pub metadata: HashMap<String, String>,
}

impl From<FunctionRecord> for FunctionMetadata {
    fn from(value: FunctionRecord) -> Self {
        Self {
            id: value.id,
            name: value.name,
            path: value.path,
            created_at: value.created_at,
            last_modified: value.last_modified,
            options: value.options,
            metadata: value.metadata,
        }
    }
}

/// Metadata for a function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionRecord {
    /// Unique identifier for the function
    pub id: FunctionId,
    /// Name of the function
    pub name: String,
    /// Path of the function within the namespace hierarchy
    pub path: String,
    /// Version of the Function Record
    pub version: u64,
    /// Timestamp when the function was created
    pub created_at: Timestamp,
    /// Timestamp when the function was last modified
    pub last_modified: Timestamp,
    /// Additional engine-specific options
    pub options: HashMap<String, String>,
    /// Function Metadata (Informative)
    pub metadata: HashMap<String, String>,
}
