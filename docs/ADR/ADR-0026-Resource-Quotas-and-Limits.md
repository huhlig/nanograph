---
parent: ADR
nav_order: 0026
title: Resource Quotas and Limits
status: proposed
date: 2026-01-11
deciders: Hans W. Uhlig
---

# ADR-0026: Resource Quotas and Limits

## Status

Proposed

## Context

In a multi-tenant environment (ADR-0025), resource management is critical to:

1. **Prevent Resource Exhaustion** - One tenant shouldn't consume all resources
2. **Ensure Fair Sharing** - Resources distributed according to tenant tier
3. **Enable Predictable Performance** - Tenants get guaranteed minimums
4. **Support Business Models** - Different pricing tiers with different limits
5. **Protect System Stability** - Prevent cascading failures from overload

Without quotas and limits:
- Noisy neighbor problems affect all tenants
- No way to enforce SLA guarantees
- Difficult to predict capacity needs
- System vulnerable to abuse or bugs
- Cannot implement tiered pricing

## Decision

Implement a **comprehensive quota and limit system** with:

1. **Multi-dimensional quotas** - Storage, compute, connections, IOPS
2. **Hierarchical enforcement** - Cluster → Tenant → Database → Table
3. **Soft and hard limits** - Warnings before enforcement
4. **Real-time tracking** - Low-overhead metering
5. **Graceful degradation** - Throttling before rejection

## Decision Drivers

* **Fairness** - Prevent resource monopolization
* **Predictability** - Tenants know their limits
* **Performance** - Low overhead tracking
* **Flexibility** - Different limits for different tiers
* **Safety** - Protect system from overload
* **Economics** - Enable usage-based pricing

## Design

### 1. Quota Types

```rust
// nanograph-kvt/src/types.rs

/// Comprehensive resource quotas
#[derive(Debug, Clone)]
pub struct ResourceQuotas {
    /// Storage quotas
    pub storage: StorageQuotas,
    
    /// Compute quotas
    pub compute: ComputeQuotas,
    
    /// Connection quotas
    pub connection: ConnectionQuotas,
    
    /// Rate limits
    pub rate_limits: RateLimits,
}

/// Storage-related quotas
#[derive(Debug, Clone)]
pub struct StorageQuotas {
    /// Maximum total storage in bytes
    pub max_storage_bytes: Option<u64>,
    
    /// Maximum number of databases
    pub max_databases: Option<u32>,
    
    /// Maximum number of tables per database
    pub max_tables_per_database: Option<u32>,
    
    /// Maximum number of shards
    pub max_shards: Option<u32>,
    
    /// Maximum key size in bytes
    pub max_key_size: Option<u32>,
    
    /// Maximum value size in bytes
    pub max_value_size: Option<u32>,
    
    /// Maximum batch size (number of operations)
    pub max_batch_size: Option<u32>,
}

/// Compute-related quotas
#[derive(Debug, Clone)]
pub struct ComputeQuotas {
    /// Maximum query execution time in milliseconds
    pub max_query_time_ms: Option<u64>,
    
    /// Maximum memory per query in bytes
    pub max_query_memory_bytes: Option<u64>,
    
    /// Maximum concurrent queries
    pub max_concurrent_queries: Option<u32>,
    
    /// Maximum scan range size
    pub max_scan_range: Option<u64>,
    
    /// Maximum result set size
    pub max_result_set_size: Option<u64>,
}

/// Connection-related quotas
#[derive(Debug, Clone)]
pub struct ConnectionQuotas {
    /// Maximum concurrent connections
    pub max_connections: Option<u32>,
    
    /// Maximum connection idle time in seconds
    pub max_idle_time_secs: Option<u64>,
    
    /// Maximum connections per IP
    pub max_connections_per_ip: Option<u32>,
}

/// Rate limiting configuration
#[derive(Debug, Clone)]
pub struct RateLimits {
    /// Maximum reads per second
    pub max_reads_per_second: Option<u32>,
    
    /// Maximum writes per second
    pub max_writes_per_second: Option<u32>,
    
    /// Maximum IOPS (total operations per second)
    pub max_iops: Option<u32>,
    
    /// Maximum bandwidth in bytes per second
    pub max_bandwidth_bytes_per_second: Option<u64>,
    
    /// Burst allowance (operations)
    pub burst_allowance: Option<u32>,
}
```

### 2. Quota Enforcement Levels

```rust
/// Where quotas are enforced in the hierarchy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaScope {
    /// Cluster-wide limits (system protection)
    Cluster,
    
    /// Per-tenant limits (primary enforcement)
    Tenant,
    
    /// Per-database limits (within tenant)
    Database,
    
    /// Per-table limits (fine-grained control)
    Table,
}

/// Quota enforcement action
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnforcementAction {
    /// Allow operation
    Allow,
    
    /// Throttle operation (slow down)
    Throttle { delay_ms: u64 },
    
    /// Reject operation with error
    Reject { reason: QuotaViolation },
    
    /// Warn but allow (soft limit)
    Warn { message: &'static str },
}

/// Types of quota violations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaViolation {
    StorageExceeded,
    ConnectionLimitReached,
    RateLimitExceeded,
    QueryTimeoutExceeded,
    MemoryLimitExceeded,
    BatchSizeExceeded,
    ConcurrentQueryLimitReached,
}
```

### 3. Usage Tracking

```rust
/// Real-time usage tracking per tenant
pub struct UsageTracker {
    /// Per-tenant usage metrics
    tenant_usage: Arc<DashMap<TenantId, TenantUsage>>,
    
    /// Aggregation interval for rate limiting
    window_duration: Duration,
}

/// Current usage for a tenant
#[derive(Debug)]
pub struct TenantUsage {
    /// Storage metrics
    pub storage_bytes: AtomicU64,
    pub database_count: AtomicU32,
    pub table_count: AtomicU32,
    pub shard_count: AtomicU32,
    
    /// Connection metrics
    pub active_connections: AtomicU32,
    pub connections_by_ip: DashMap<IpAddr, u32>,
    
    /// Compute metrics
    pub active_queries: AtomicU32,
    pub total_query_time_ms: AtomicU64,
    pub peak_memory_bytes: AtomicU64,
    
    /// Rate limiting (sliding window)
    pub reads_per_second: RateLimiter,
    pub writes_per_second: RateLimiter,
    pub total_iops: RateLimiter,
    pub bandwidth_bytes: RateLimiter,
    
    /// Last update timestamp
    pub last_updated: AtomicU64,
}

impl UsageTracker {
    /// Check if operation is allowed under quotas
    pub fn check_quota(
        &self,
        tenant: TenantId,
        operation: &Operation,
        quotas: &ResourceQuotas,
    ) -> EnforcementAction {
        let usage = self.get_or_create_usage(tenant);
        
        match operation {
            Operation::Write { key, value } => {
                // Check storage quota
                if let Some(max) = quotas.storage.max_storage_bytes {
                    let current = usage.storage_bytes.load(Ordering::Relaxed);
                    let size = key.len() + value.len();
                    
                    if current + size as u64 > max {
                        return EnforcementAction::Reject {
                            reason: QuotaViolation::StorageExceeded,
                        };
                    }
                }
                
                // Check write rate limit
                if let Some(max_writes) = quotas.rate_limits.max_writes_per_second {
                    if !usage.writes_per_second.check_rate(max_writes) {
                        return EnforcementAction::Throttle { delay_ms: 100 };
                    }
                }
                
                // Check key/value size limits
                if let Some(max_key) = quotas.storage.max_key_size {
                    if key.len() > max_key as usize {
                        return EnforcementAction::Reject {
                            reason: QuotaViolation::BatchSizeExceeded,
                        };
                    }
                }
                
                EnforcementAction::Allow
            }
            
            Operation::Read { .. } => {
                // Check read rate limit
                if let Some(max_reads) = quotas.rate_limits.max_reads_per_second {
                    if !usage.reads_per_second.check_rate(max_reads) {
                        return EnforcementAction::Throttle { delay_ms: 50 };
                    }
                }
                
                EnforcementAction::Allow
            }
            
            Operation::Query { .. } => {
                // Check concurrent query limit
                if let Some(max_queries) = quotas.compute.max_concurrent_queries {
                    let current = usage.active_queries.load(Ordering::Relaxed);
                    if current >= max_queries {
                        return EnforcementAction::Reject {
                            reason: QuotaViolation::ConcurrentQueryLimitReached,
                        };
                    }
                }
                
                EnforcementAction::Allow
            }
            
            Operation::Connect { ip } => {
                // Check connection limit
                if let Some(max_conn) = quotas.connection.max_connections {
                    let current = usage.active_connections.load(Ordering::Relaxed);
                    if current >= max_conn {
                        return EnforcementAction::Reject {
                            reason: QuotaViolation::ConnectionLimitReached,
                        };
                    }
                }
                
                // Check per-IP limit
                if let Some(max_per_ip) = quotas.connection.max_connections_per_ip {
                    let ip_count = usage.connections_by_ip
                        .entry(ip)
                        .or_insert(0);
                    
                    if *ip_count >= max_per_ip {
                        return EnforcementAction::Reject {
                            reason: QuotaViolation::ConnectionLimitReached,
                        };
                    }
                }
                
                EnforcementAction::Allow
            }
        }
    }
    
    /// Record operation for usage tracking
    pub fn record_operation(
        &self,
        tenant: TenantId,
        operation: &Operation,
    ) {
        let usage = self.get_or_create_usage(tenant);
        
        match operation {
            Operation::Write { key, value } => {
                let size = (key.len() + value.len()) as u64;
                usage.storage_bytes.fetch_add(size, Ordering::Relaxed);
                usage.writes_per_second.record();
                usage.total_iops.record();
                usage.bandwidth_bytes.record(size);
            }
            Operation::Read { .. } => {
                usage.reads_per_second.record();
                usage.total_iops.record();
            }
            Operation::Query { duration_ms, memory_bytes } => {
                usage.total_query_time_ms.fetch_add(*duration_ms, Ordering::Relaxed);
                usage.peak_memory_bytes.fetch_max(*memory_bytes, Ordering::Relaxed);
            }
            Operation::Connect { ip } => {
                usage.active_connections.fetch_add(1, Ordering::Relaxed);
                *usage.connections_by_ip.entry(*ip).or_insert(0) += 1;
            }
        }
        
        usage.last_updated.store(
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            Ordering::Relaxed,
        );
    }
}
```

### 4. Rate Limiting Implementation

```rust
/// Token bucket rate limiter
pub struct RateLimiter {
    /// Maximum tokens (burst capacity)
    capacity: u32,
    
    /// Current token count
    tokens: AtomicU32,
    
    /// Refill rate (tokens per second)
    refill_rate: u32,
    
    /// Last refill timestamp
    last_refill: AtomicU64,
}

impl RateLimiter {
    pub fn new(capacity: u32, refill_rate: u32) -> Self {
        Self {
            capacity,
            tokens: AtomicU32::new(capacity),
            refill_rate,
            last_refill: AtomicU64::new(
                SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
            ),
        }
    }
    
    /// Check if operation is allowed (consumes 1 token)
    pub fn check_rate(&self, max_rate: u32) -> bool {
        self.refill();
        
        let current = self.tokens.load(Ordering::Relaxed);
        if current > 0 {
            self.tokens.fetch_sub(1, Ordering::Relaxed);
            true
        } else {
            false
        }
    }
    
    /// Refill tokens based on elapsed time
    fn refill(&self) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
        let last = self.last_refill.load(Ordering::Relaxed);
        let elapsed_ms = now.saturating_sub(last);
        
        if elapsed_ms > 0 {
            let tokens_to_add = (elapsed_ms * self.refill_rate as u64) / 1000;
            if tokens_to_add > 0 {
                let current = self.tokens.load(Ordering::Relaxed);
                let new_tokens = (current + tokens_to_add as u32).min(self.capacity);
                self.tokens.store(new_tokens, Ordering::Relaxed);
                self.last_refill.store(now, Ordering::Relaxed);
            }
        }
    }
    
    /// Record an operation (for tracking only)
    pub fn record(&self) {
        // Update metrics
    }
}
```

### 5. Quota Configuration Tiers

```rust
/// Predefined quota tiers for different customer segments
pub enum QuotaTier {
    Free,
    Starter,
    Professional,
    Enterprise,
    Custom(ResourceQuotas),
}

impl QuotaTier {
    pub fn to_quotas(&self) -> ResourceQuotas {
        match self {
            QuotaTier::Free => ResourceQuotas {
                storage: StorageQuotas {
                    max_storage_bytes: Some(100 * 1024 * 1024), // 100 MB
                    max_databases: Some(1),
                    max_tables_per_database: Some(10),
                    max_shards: Some(1),
                    max_key_size: Some(1024),
                    max_value_size: Some(64 * 1024),
                    max_batch_size: Some(100),
                },
                compute: ComputeQuotas {
                    max_query_time_ms: Some(5000),
                    max_query_memory_bytes: Some(100 * 1024 * 1024),
                    max_concurrent_queries: Some(5),
                    max_scan_range: Some(1000),
                    max_result_set_size: Some(10 * 1024 * 1024),
                },
                connection: ConnectionQuotas {
                    max_connections: Some(10),
                    max_idle_time_secs: Some(300),
                    max_connections_per_ip: Some(5),
                },
                rate_limits: RateLimits {
                    max_reads_per_second: Some(100),
                    max_writes_per_second: Some(50),
                    max_iops: Some(150),
                    max_bandwidth_bytes_per_second: Some(1024 * 1024),
                    burst_allowance: Some(200),
                },
            },
            
            QuotaTier::Starter => ResourceQuotas {
                storage: StorageQuotas {
                    max_storage_bytes: Some(10 * 1024 * 1024 * 1024), // 10 GB
                    max_databases: Some(5),
                    max_tables_per_database: Some(50),
                    max_shards: Some(10),
                    max_key_size: Some(4096),
                    max_value_size: Some(1024 * 1024),
                    max_batch_size: Some(1000),
                },
                compute: ComputeQuotas {
                    max_query_time_ms: Some(30000),
                    max_query_memory_bytes: Some(1024 * 1024 * 1024),
                    max_concurrent_queries: Some(25),
                    max_scan_range: Some(10000),
                    max_result_set_size: Some(100 * 1024 * 1024),
                },
                connection: ConnectionQuotas {
                    max_connections: Some(100),
                    max_idle_time_secs: Some(600),
                    max_connections_per_ip: Some(50),
                },
                rate_limits: RateLimits {
                    max_reads_per_second: Some(1000),
                    max_writes_per_second: Some(500),
                    max_iops: Some(1500),
                    max_bandwidth_bytes_per_second: Some(10 * 1024 * 1024),
                    burst_allowance: Some(2000),
                },
            },
            
            QuotaTier::Professional => ResourceQuotas {
                storage: StorageQuotas {
                    max_storage_bytes: Some(100 * 1024 * 1024 * 1024), // 100 GB
                    max_databases: Some(25),
                    max_tables_per_database: Some(200),
                    max_shards: Some(50),
                    max_key_size: Some(8192),
                    max_value_size: Some(10 * 1024 * 1024),
                    max_batch_size: Some(10000),
                },
                compute: ComputeQuotas {
                    max_query_time_ms: Some(60000),
                    max_query_memory_bytes: Some(4 * 1024 * 1024 * 1024),
                    max_concurrent_queries: Some(100),
                    max_scan_range: Some(100000),
                    max_result_set_size: Some(1024 * 1024 * 1024),
                },
                connection: ConnectionQuotas {
                    max_connections: Some(500),
                    max_idle_time_secs: Some(1800),
                    max_connections_per_ip: Some(250),
                },
                rate_limits: RateLimits {
                    max_reads_per_second: Some(10000),
                    max_writes_per_second: Some(5000),
                    max_iops: Some(15000),
                    max_bandwidth_bytes_per_second: Some(100 * 1024 * 1024),
                    burst_allowance: Some(20000),
                },
            },
            
            QuotaTier::Enterprise => ResourceQuotas {
                storage: StorageQuotas {
                    max_storage_bytes: None, // Unlimited
                    max_databases: None,
                    max_tables_per_database: None,
                    max_shards: None,
                    max_key_size: Some(65536),
                    max_value_size: Some(100 * 1024 * 1024),
                    max_batch_size: Some(100000),
                },
                compute: ComputeQuotas {
                    max_query_time_ms: None,
                    max_query_memory_bytes: None,
                    max_concurrent_queries: None,
                    max_scan_range: None,
                    max_result_set_size: None,
                },
                connection: ConnectionQuotas {
                    max_connections: None,
                    max_idle_time_secs: Some(3600),
                    max_connections_per_ip: None,
                },
                rate_limits: RateLimits {
                    max_reads_per_second: None,
                    max_writes_per_second: None,
                    max_iops: None,
                    max_bandwidth_bytes_per_second: None,
                    burst_allowance: Some(100000),
                },
            },
            
            QuotaTier::Custom(quotas) => quotas.clone(),
        }
    }
}
```

### 6. Monitoring and Alerting

```rust
/// Quota monitoring and alerting
pub struct QuotaMonitor {
    usage_tracker: Arc<UsageTracker>,
    alert_thresholds: AlertThresholds,
}

#[derive(Debug, Clone)]
pub struct AlertThresholds {
    /// Warn at 80% of quota
    pub warning_threshold: f64,
    
    /// Critical at 95% of quota
    pub critical_threshold: f64,
}

impl QuotaMonitor {
    /// Check if tenant is approaching quota limits
    pub fn check_thresholds(&self, tenant: TenantId, quotas: &ResourceQuotas) -> Vec<Alert> {
        let usage = self.usage_tracker.get_usage(tenant);
        let mut alerts = Vec::new();
        
        // Check storage quota
        if let Some(max_storage) = quotas.storage.max_storage_bytes {
            let current = usage.storage_bytes.load(Ordering::Relaxed);
            let percentage = (current as f64 / max_storage as f64) * 100.0;
            
            if percentage >= self.alert_thresholds.critical_threshold {
                alerts.push(Alert {
                    tenant,
                    severity: AlertSeverity::Critical,
                    resource: ResourceType::Storage,
                    message: format!("Storage at {:.1}% of quota", percentage),
                });
            } else if percentage >= self.alert_thresholds.warning_threshold {
                alerts.push(Alert {
                    tenant,
                    severity: AlertSeverity::Warning,
                    resource: ResourceType::Storage,
                    message: format!("Storage at {:.1}% of quota", percentage),
                });
            }
        }
        
        // Check other quotas...
        
        alerts
    }
}

#[derive(Debug)]
pub struct Alert {
    pub tenant: TenantId,
    pub severity: AlertSeverity,
    pub resource: ResourceType,
    pub message: String,
}

#[derive(Debug, Clone, Copy)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Copy)]
pub enum ResourceType {
    Storage,
    Connections,
    Queries,
    RateLimit,
}
```

## Consequences

### Positive

* **Fair Resource Sharing** - Prevents monopolization
* **Predictable Performance** - Tenants know their limits
* **System Protection** - Prevents overload and cascading failures
* **Business Enablement** - Supports tiered pricing models
* **Operational Visibility** - Clear usage metrics
* **Graceful Degradation** - Throttling before rejection
* **Flexibility** - Different limits for different needs

### Negative

* **Overhead** - Tracking adds latency (mitigated by atomic operations)
* **Complexity** - Many quota types to manage
* **False Positives** - Burst traffic may hit limits
* **Configuration Burden** - Must tune quotas appropriately

### Risks

* **Tracking Bugs** - Incorrect usage accounting
* **Performance Impact** - Quota checks on hot path
* **Quota Gaming** - Tenants may try to circumvent limits
* **Cascading Throttling** - Throttling one tenant affects others

## Alternatives Considered

### 1. No Quotas (Trust-Based)

**Rejected** - Vulnerable to abuse, no protection against bugs or malicious actors.

### 2. Simple Storage-Only Quotas

**Rejected** - Insufficient for multi-dimensional resource management.

### 3. External Rate Limiting (API Gateway)

**Rejected** - Cannot enforce internal resource limits, adds latency.

### 4. Reactive Throttling Only

**Rejected** - No proactive protection, difficult to predict behavior.

## Implementation Notes

### Performance Optimization

- Use atomic operations for counters (lock-free)
- Batch quota checks where possible
- Cache quota configurations in memory
- Use approximate counting for high-frequency operations
- Implement fast-path for unlimited quotas

### Testing Strategy

- Load testing with quota enforcement
- Chaos testing (quota violations)
- Tenant isolation verification
- Performance benchmarks with quotas enabled

## Related ADRs

* [ADR-0025: Multi-Tenancy and Isolation](ADR-0025-Multi-Tenancy-and-Isolation.md)
* [ADR-0011: Observability, Telemetry, and Auditing](ADR-0011-Observability-Telemetry-and-Auditing.md)
* [ADR-0013: Memory Management and Caching Strategy](ADR-0013-Memory-Management-and-Caching-Strategy.md)

## References

* AWS DynamoDB capacity modes
* Google Cloud Spanner quotas
* Azure Cosmos DB request units
* Stripe API rate limiting
* Token bucket algorithm

---

**Next Steps:**
1. Implement usage tracking infrastructure
2. Add quota enforcement to database manager
3. Create monitoring dashboards
4. Define default quota tiers
5. Add quota management APIs