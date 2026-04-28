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

//! Spatial index traits for geometric queries

use crate::error::IndexResult;
use crate::index::{IndexEntry, IndexStore};
use async_trait::async_trait;

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

    /// Calculate Manhattan distance to another point
    pub fn manhattan_distance_to(&self, other: &Point) -> f64 {
        (self.x - other.x).abs() + (self.y - other.y).abs()
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

    /// Expand box to include a point
    pub fn expand_to_include(&mut self, point: &Point) {
        self.min.x = self.min.x.min(point.x);
        self.min.y = self.min.y.min(point.y);
        self.max.x = self.max.x.max(point.x);
        self.max.y = self.max.y.max(point.y);
    }
}

/// Entry with distance information
#[derive(Debug, Clone)]
pub struct DistancedEntry {
    /// The index entry
    pub entry: IndexEntry,
    /// Distance from query point
    pub distance: f64,
}

/// Distance metric for spatial queries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistanceMetric {
    /// Euclidean distance (L2 norm)
    Euclidean,
    /// Manhattan distance (L1 norm)
    Manhattan,
    /// Haversine distance (great-circle distance on sphere)
    Haversine,
}

/// Trait for spatial indexes
#[async_trait]
pub trait SpatialIndex: IndexStore {
    /// Query by bounding box
    ///
    /// # Arguments
    /// * `bbox` - The bounding box to search within
    ///
    /// # Returns
    /// * `Ok(Vec<IndexEntry>)` - All entries within the bounding box
    async fn query_bbox(&self, bbox: BoundingBox) -> IndexResult<Vec<IndexEntry>>;

    /// K-nearest neighbors search
    ///
    /// # Arguments
    /// * `point` - The query point
    /// * `k` - Number of nearest neighbors to return
    ///
    /// # Returns
    /// * `Ok(Vec<DistancedEntry>)` - K nearest entries sorted by distance
    async fn query_knn(&self, point: Point, k: usize) -> IndexResult<Vec<DistancedEntry>>;

    /// Radius search (all points within distance)
    ///
    /// # Arguments
    /// * `center` - The center point
    /// * `radius` - Maximum distance from center
    ///
    /// # Returns
    /// * `Ok(Vec<DistancedEntry>)` - All entries within radius
    async fn query_radius(&self, center: Point, radius: f64) -> IndexResult<Vec<DistancedEntry>>;

    /// Point-in-polygon test
    ///
    /// # Arguments
    /// * `polygon` - Vertices of the polygon (closed)
    ///
    /// # Returns
    /// * `Ok(Vec<IndexEntry>)` - All entries inside the polygon
    async fn query_polygon(&self, polygon: &[Point]) -> IndexResult<Vec<IndexEntry>>;

    /// Get distance metric used by this index
    fn distance_metric(&self) -> DistanceMetric;

    /// Get spatial statistics
    async fn spatial_stats(&self) -> IndexResult<SpatialStats>;
}

/// Statistics for spatial index
#[derive(Debug, Clone)]
pub struct SpatialStats {
    /// Total number of points
    pub point_count: u64,
    /// Bounding box of all points
    pub total_bounds: BoundingBox,
    /// Average density (points per unit area)
    pub avg_density: f64,
    /// Number of R-Tree nodes (if applicable)
    pub node_count: Option<u64>,
}


