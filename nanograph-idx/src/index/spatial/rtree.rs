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

//! Spatial index implementation using R-Tree
//!
//! Spatial indexes are ideal for:
//! - Geographic queries (point-in-polygon, bounding box)
//! - Nearest neighbor search
//! - Distance calculations
//! - Geometric operations

use crate::error::{IndexError, IndexResult};
use crate::store::{IndexEntry, IndexQuery, IndexStats, IndexStore};
use async_trait::async_trait;
use nanograph_core::object::{IndexId, IndexRecord};
use std::collections::HashMap;

/// A point in 2D space
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Calculate Euclidean distance to another point
    pub fn distance_to(&self, other: &Point) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

/// A bounding box in 2D space
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundingBox {
    pub min: Point,
    pub max: Point,
}

impl BoundingBox {
    pub fn new(min: Point, max: Point) -> Self {
        Self { min, max }
    }

    /// Check if this box contains a point
    pub fn contains(&self, point: &Point) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }

    /// Check if this box intersects another box
    pub fn intersects(&self, other: &BoundingBox) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
    }

    /// Calculate the area of this box
    pub fn area(&self) -> f64 {
        (self.max.x - self.min.x) * (self.max.y - self.min.y)
    }
}

/// Spatial index implementation using R-Tree
///
/// This index uses an R-Tree structure to efficiently index spatial data,
/// enabling fast geographic and geometric queries.
///
/// # Example
///
/// ```ignore
/// use nanograph_idx::SpatialIndex;
/// use nanograph_core::object::{IndexCreate, IndexType};
///
/// let config = IndexCreate::new(
///     "locations_coords_idx",
///     IndexType::Spatial,
///     vec!["latitude".to_string(), "longitude".to_string()],
/// );
///
/// let index = SpatialIndex::new(metadata)?;
/// ```
pub struct RTreeSpatialIndex {
    /// Index metadata
    metadata: IndexRecord,
    /// In-memory spatial index (TODO: Replace with persistent R-Tree)
    /// Maps primary_key -> (point, bounding_box)
    entries: HashMap<Vec<u8>, (Point, BoundingBox)>,
}

impl RTreeSpatialIndex {
    /// Create a new spatial index
    pub fn new(metadata: IndexRecord) -> IndexResult<Self> {
        Ok(Self {
            metadata,
            entries: HashMap::new(),
        })
    }

    /// Parse coordinates from indexed value
    fn parse_coordinates(&self, value: &[u8]) -> IndexResult<Point> {
        // TODO: Implement proper coordinate parsing
        // Expected format: [latitude, longitude] as f64 bytes
        if value.len() != 16 {
            return Err(IndexError::InvalidConfig(
                "Spatial index requires 16 bytes (2 x f64) for coordinates".to_string(),
            ));
        }

        let lat_bytes: [u8; 8] = value[0..8].try_into().unwrap();
        let lon_bytes: [u8; 8] = value[8..16].try_into().unwrap();

        let lat = f64::from_be_bytes(lat_bytes);
        let lon = f64::from_be_bytes(lon_bytes);

        Ok(Point::new(lat, lon))
    }

    /// Find entries within a bounding box
    pub async fn query_bbox(&self, bbox: BoundingBox) -> IndexResult<Vec<IndexEntry>> {
        let mut results = Vec::new();

        for (primary_key, (point, _)) in &self.entries {
            if bbox.contains(point) {
                // Serialize point back to bytes
                let mut value = Vec::with_capacity(16);
                value.extend_from_slice(&point.x.to_be_bytes());
                value.extend_from_slice(&point.y.to_be_bytes());

                results.push(IndexEntry {
                    indexed_value: value,
                    primary_key: primary_key.clone(),
                    included_columns: None,
                });
            }
        }

        Ok(results)
    }

    /// Find k nearest neighbors to a point
    pub async fn query_knn(&self, point: Point, k: usize) -> IndexResult<Vec<IndexEntry>> {
        // TODO: Implement efficient k-NN search using R-Tree
        // Current implementation is naive O(n) scan
        let mut distances: Vec<(f64, Vec<u8>, Point)> = self
            .entries
            .iter()
            .map(|(pk, (p, _))| (point.distance_to(p), pk.clone(), *p))
            .collect();

        distances.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        distances.truncate(k);

        Ok(distances
            .into_iter()
            .map(|(_, primary_key, p)| {
                let mut value = Vec::with_capacity(16);
                value.extend_from_slice(&p.x.to_be_bytes());
                value.extend_from_slice(&p.y.to_be_bytes());

                IndexEntry {
                    indexed_value: value,
                    primary_key,
                    included_columns: None,
                }
            })
            .collect())
    }
}

#[async_trait]
impl IndexStore for RTreeSpatialIndex {
    fn metadata(&self) -> &IndexRecord {
        &self.metadata
    }

    async fn build<I>(&mut self, _table_data: I) -> IndexResult<()>
    where
        I: Iterator<Item = (Vec<u8>, Vec<u8>)> + Send,
    {
        // TODO: Implement index building
        // 1. Extract coordinates from table rows
        // 2. Build R-Tree structure
        // 3. Update index status to Active
        Err(IndexError::BuildFailed(
            "Spatial index building not yet implemented".to_string(),
        ))
    }

    async fn insert(&mut self, entry: IndexEntry) -> IndexResult<()> {
        // TODO: Implement insertion into R-Tree
        let point = self.parse_coordinates(&entry.indexed_value)?;
        
        // Create a small bounding box around the point
        let bbox = BoundingBox::new(
            Point::new(point.x - 0.0001, point.y - 0.0001),
            Point::new(point.x + 0.0001, point.y + 0.0001),
        );

        self.entries.insert(entry.primary_key, (point, bbox));
        Ok(())
    }

    async fn delete(&mut self, primary_key: &[u8]) -> IndexResult<()> {
        // TODO: Implement deletion from R-Tree
        self.entries.remove(primary_key);
        Ok(())
    }

    async fn query(&self, _query: IndexQuery) -> IndexResult<Vec<IndexEntry>> {
        // TODO: Implement spatial query
        // Parse query bounds as bounding box and use query_bbox
        Err(IndexError::QueryFailed(
            "Spatial index queries not yet implemented".to_string(),
        ))
    }

    async fn get(&self, primary_key: &[u8]) -> IndexResult<Option<IndexEntry>> {
        if let Some((point, _)) = self.entries.get(primary_key) {
            let mut value = Vec::with_capacity(16);
            value.extend_from_slice(&point.x.to_be_bytes());
            value.extend_from_slice(&point.y.to_be_bytes());

            Ok(Some(IndexEntry {
                indexed_value: value,
                primary_key: primary_key.to_vec(),
                included_columns: None,
            }))
        } else {
            Ok(None)
        }
    }

    async fn exists(&self, indexed_value: &[u8]) -> IndexResult<bool> {
        // Check if any entry has these exact coordinates
        let point = self.parse_coordinates(indexed_value)?;
        Ok(self.entries.values().any(|(p, _)| *p == point))
    }

    async fn stats(&self) -> IndexResult<IndexStats> {
        // TODO: Calculate accurate statistics
        Ok(IndexStats {
            entry_count: self.entries.len() as u64,
            size_bytes: 0, // TODO: Calculate actual size
            levels: None,  // TODO: Calculate R-Tree depth
            avg_entry_size: 16, // 2 x f64
            fragmentation: None,
        })
    }

    async fn optimize(&mut self) -> IndexResult<()> {
        // TODO: Implement R-Tree optimization
        // 1. Rebalance tree
        // 2. Minimize bounding box overlaps
        // 3. Compact storage
        Ok(())
    }

    async fn flush(&mut self) -> IndexResult<()> {
        // TODO: Implement flush
        // 1. Write pending changes to storage
        // 2. Sync to disk
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nanograph_core::object::{IndexStatus, IndexType};
    use nanograph_core::types::Timestamp;
    use std::collections::HashMap as StdHashMap;

    fn create_test_metadata() -> IndexRecord {
        IndexRecord {
            id: IndexId::new(1),
            name: "test_spatial_idx".to_string(),
            version: 0,
            index_type: IndexType::Spatial,
            created_at: Timestamp::now(),
            last_modified: Timestamp::now(),
            columns: vec!["latitude".to_string(), "longitude".to_string()],
            key_extractor: None,
            options: StdHashMap::new(),
            metadata: StdHashMap::new(),
            status: IndexStatus::Building,
        }
    }

    #[test]
    fn test_point_distance() {
        let p1 = Point::new(0.0, 0.0);
        let p2 = Point::new(3.0, 4.0);
        assert_eq!(p1.distance_to(&p2), 5.0);
    }

    #[test]
    fn test_bounding_box_contains() {
        let bbox = BoundingBox::new(Point::new(0.0, 0.0), Point::new(10.0, 10.0));
        assert!(bbox.contains(&Point::new(5.0, 5.0)));
        assert!(!bbox.contains(&Point::new(15.0, 5.0)));
    }

    #[test]
    fn test_bounding_box_intersects() {
        let bbox1 = BoundingBox::new(Point::new(0.0, 0.0), Point::new(10.0, 10.0));
        let bbox2 = BoundingBox::new(Point::new(5.0, 5.0), Point::new(15.0, 15.0));
        let bbox3 = BoundingBox::new(Point::new(20.0, 20.0), Point::new(30.0, 30.0));

        assert!(bbox1.intersects(&bbox2));
        assert!(!bbox1.intersects(&bbox3));
    }

    #[tokio::test]
    async fn test_spatial_index_creation() {
        let metadata = create_test_metadata();
        let index = SpatialIndex::new(metadata);
        assert!(index.is_ok());
    }

    #[tokio::test]
    async fn test_spatial_index_insert() {
        let metadata = create_test_metadata();
        let mut index = SpatialIndex::new(metadata).unwrap();

        // Create coordinates: latitude=37.7749, longitude=-122.4194 (San Francisco)
        let mut coords = Vec::new();
        coords.extend_from_slice(&37.7749f64.to_be_bytes());
        coords.extend_from_slice(&(-122.4194f64).to_be_bytes());

        let entry = IndexEntry {
            indexed_value: coords,
            primary_key: b"location1".to_vec(),
            included_columns: None,
        };

        let result = index.insert(entry).await;
        assert!(result.is_ok());
    }
}

// Made with Bob
