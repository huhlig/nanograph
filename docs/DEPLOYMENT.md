# Nanograph Deployment Guide

**Version:** 1.0  
**Date:** 2026-01-07  
**Status:** Active

---

## Table of Contents

- [Overview](#overview)
- [Deployment Modes](#deployment-modes)
- [Embedded Mode](#embedded-mode)
- [Standalone Mode](#standalone-mode)
- [Cluster Mode](#cluster-mode)
- [Configuration](#configuration)
- [Security](#security)
- [Monitoring](#monitoring)
- [Backup and Recovery](#backup-and-recovery)
- [Performance Tuning](#performance-tuning)
- [Troubleshooting](#troubleshooting)
- [Production Checklist](#production-checklist)

---

## Overview

Nanograph supports three deployment modes, each optimized for different use cases:

1. **Embedded Mode:** Library embedded in your application
2. **Standalone Mode:** Single-node server deployment
3. **Cluster Mode:** Multi-node distributed deployment

### Deployment Decision Matrix

| Requirement | Embedded | Standalone | Cluster |
|-------------|----------|------------|---------|
| Simplicity | ✅ Best | ✅ Good | ⚠️ Complex |
| Performance | ✅ Best | ✅ Good | ✅ Good |
| High Availability | ❌ No | ❌ No | ✅ Yes |
| Horizontal Scaling | ❌ No | ❌ No | ✅ Yes |
| Resource Isolation | ⚠️ Shared | ✅ Isolated | ✅ Isolated |
| Operational Overhead | ✅ Minimal | ⚠️ Moderate | ❌ High |

---

## Deployment Modes

### Embedded Mode

**Best For:**
- Desktop applications
- Mobile applications
- Edge devices
- Single-tenant applications
- Development and testing

**Characteristics:**
- In-process database
- No network overhead
- Shared memory with application
- Automatic lifecycle management

**Limitations:**
- Single process access
- No remote access
- Limited to application resources

### Standalone Mode

**Best For:**
- Small to medium applications
- Single-server deployments
- Development environments
- Low-traffic production workloads

**Characteristics:**
- Separate server process
- Network-based access
- Multiple client connections
- Independent resource management

**Limitations:**
- Single point of failure
- Vertical scaling only
- No automatic failover

### Cluster Mode

**Best For:**
- High-availability requirements
- Large-scale applications
- Multi-tenant systems
- Production workloads with SLAs

**Characteristics:**
- Multiple server nodes
- Automatic failover
- Horizontal scaling
- Data replication
- Consensus-based coordination

**Complexity:**
- Requires cluster management
- Network configuration
- Monitoring and alerting
- Operational expertise

---

## Embedded Mode

### Quick Start

#### Rust Application

```rust
use nanograph::{Database, Config};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create configuration
    let config = Config::builder()
        .data_dir("./data")
        .cache_size_mb(256)
        .build()?;
    
    // Open database
    let db = Database::open(config)?;
    
    // Use database
    db.put(b"key", b"value")?;
    let value = db.get(b"key")?;
    
    Ok(())
}
```

#### JavaScript/TypeScript Application

```typescript
import { Database, Config } from '@nanograph/client';

async function main() {
  // Create configuration
  const config = new Config({
    dataDir: './data',
    cacheSizeMb: 256
  });
  
  // Open database
  const db = await Database.open(config);
  
  // Use database
  await db.put('key', 'value');
  const value = await db.get('key');
  
  await db.close();
}
```

### Configuration

```toml
# nanograph.toml
[storage]
data_dir = "./data"
wal_dir = "./wal"
max_file_size_mb = 64

[memory]
cache_size_mb = 256
memtable_size_mb = 64
block_cache_mb = 128

[performance]
write_buffer_size = 4194304  # 4MB
max_background_jobs = 4
compaction_style = "leveled"

[durability]
sync_writes = true
wal_sync_interval_ms = 1000
```

### Resource Requirements

**Minimum:**
- RAM: 128 MB
- Disk: 100 MB
- CPU: 1 core

**Recommended:**
- RAM: 512 MB - 2 GB
- Disk: 1 GB+ (depends on data size)
- CPU: 2+ cores

### Best Practices

1. **Data Directory:** Use fast local storage (SSD preferred)
2. **Cache Sizing:** Allocate 25-50% of available RAM
3. **Write Buffer:** Larger buffers improve write throughput
4. **Compaction:** Run during off-peak hours if possible
5. **Backups:** Regular snapshots of data directory

---

## Standalone Mode

### Installation

#### From Binary

```bash
# Download latest release
wget https://github.com/nanograph/nanograph/releases/download/v1.0.0/nanograph-server-linux-amd64.tar.gz

# Extract
tar -xzf nanograph-server-linux-amd64.tar.gz

# Install
sudo mv nanograph-server /usr/local/bin/
sudo chmod +x /usr/local/bin/nanograph-server
```

#### From Source

```bash
# Clone repository
git clone https://github.com/nanograph/nanograph.git
cd nanograph

# Build release binary
cargo build --release --bin nanograph-server

# Install
sudo cp target/release/nanograph-server /usr/local/bin/
```

### Configuration

Create `/etc/nanograph/server.toml`:

```toml
[server]
bind_address = "0.0.0.0:7437"
max_connections = 1000
request_timeout_ms = 30000

[storage]
data_dir = "/var/lib/nanograph/data"
wal_dir = "/var/lib/nanograph/wal"

[memory]
cache_size_mb = 2048
memtable_size_mb = 256

[security]
tls_enabled = true
tls_cert_path = "/etc/nanograph/certs/server.crt"
tls_key_path = "/etc/nanograph/certs/server.key"
require_auth = true

[logging]
level = "info"
output = "/var/log/nanograph/server.log"
```

### Running the Server

#### Manual Start

```bash
# Start server
nanograph-server --config /etc/nanograph/server.toml

# Start with custom data directory
nanograph-server --data-dir /path/to/data

# Start in foreground (for debugging)
nanograph-server --config /etc/nanograph/server.toml --foreground
```

#### Systemd Service

Create `/etc/systemd/system/nanograph.service`:

```ini
[Unit]
Description=Nanograph Database Server
After=network.target

[Service]
Type=simple
User=nanograph
Group=nanograph
ExecStart=/usr/local/bin/nanograph-server --config /etc/nanograph/server.toml
Restart=on-failure
RestartSec=5
LimitNOFILE=65536

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/nanograph /var/log/nanograph

[Install]
WantedBy=multi-user.target
```

Enable and start:

```bash
# Create user
sudo useradd -r -s /bin/false nanograph

# Create directories
sudo mkdir -p /var/lib/nanograph/{data,wal}
sudo mkdir -p /var/log/nanograph
sudo chown -R nanograph:nanograph /var/lib/nanograph /var/log/nanograph

# Enable and start service
sudo systemctl daemon-reload
sudo systemctl enable nanograph
sudo systemctl start nanograph

# Check status
sudo systemctl status nanograph
```

### Docker Deployment

#### Dockerfile

```dockerfile
FROM rust:1.70 as builder

WORKDIR /app
COPY . .
RUN cargo build --release --bin nanograph-server

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/nanograph-server /usr/local/bin/

RUN useradd -r -s /bin/false nanograph && \
    mkdir -p /var/lib/nanograph/{data,wal} && \
    chown -R nanograph:nanograph /var/lib/nanograph

USER nanograph
EXPOSE 7437

ENTRYPOINT ["nanograph-server"]
CMD ["--config", "/etc/nanograph/server.toml"]
```

#### Docker Compose

```yaml
version: '3.8'

services:
  nanograph:
    image: nanograph/server:latest
    container_name: nanograph-server
    ports:
      - "7437:7437"
    volumes:
      - nanograph-data:/var/lib/nanograph/data
      - nanograph-wal:/var/lib/nanograph/wal
      - ./config:/etc/nanograph:ro
    environment:
      - RUST_LOG=info
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "nanograph-cli", "ping"]
      interval: 30s
      timeout: 10s
      retries: 3

volumes:
  nanograph-data:
  nanograph-wal:
```

Run with:

```bash
docker-compose up -d
```

### Resource Requirements

**Minimum:**
- RAM: 1 GB
- Disk: 10 GB
- CPU: 2 cores
- Network: 100 Mbps

**Recommended:**
- RAM: 4-8 GB
- Disk: 100 GB SSD
- CPU: 4-8 cores
- Network: 1 Gbps

---

## Cluster Mode

### Architecture Overview

A Nanograph cluster consists of:

- **Nodes:** Server instances running Nanograph
- **Shards:** Partitions of data distributed across nodes
- **Replicas:** Copies of each shard for redundancy
- **Raft Groups:** Consensus groups for each shard

### Cluster Topology

#### Minimum Production Cluster

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Node 1    │     │   Node 2    │     │   Node 3    │
│  (Leader)   │────▶│  (Follower) │────▶│  (Follower) │
│             │     │             │     │             │
│ Shard 1: L  │     │ Shard 1: F  │     │ Shard 1: F  │
│ Shard 2: F  │     │ Shard 2: L  │     │ Shard 2: F  │
│ Shard 3: F  │     │ Shard 3: F  │     │ Shard 3: L  │
└─────────────┘     └─────────────┘     └─────────────┘
```

**Characteristics:**
- 3 nodes (minimum for quorum)
- 3 shards (example)
- Replication factor: 3
- Tolerates 1 node failure

### Cluster Setup

#### 1. Prepare Nodes

On each node:

```bash
# Install Nanograph
sudo apt-get update
sudo apt-get install -y nanograph-server

# Create directories
sudo mkdir -p /var/lib/nanograph/{data,wal}
sudo mkdir -p /etc/nanograph
sudo chown -R nanograph:nanograph /var/lib/nanograph
```

#### 2. Configure Nodes

**Node 1** (`/etc/nanograph/server.toml`):

```toml
[server]
node_id = "node1"
bind_address = "0.0.0.0:7437"
advertise_address = "10.0.1.10:7437"

[cluster]
enabled = true
initial_nodes = [
    "node1=10.0.1.10:7437",
    "node2=10.0.1.11:7437",
    "node3=10.0.1.12:7437"
]
replication_factor = 3
num_shards = 16

[raft]
election_timeout_ms = 1000
heartbeat_interval_ms = 100
snapshot_interval = 10000

[storage]
data_dir = "/var/lib/nanograph/data"
wal_dir = "/var/lib/nanograph/wal"

[memory]
cache_size_mb = 4096
```

**Node 2** and **Node 3**: Similar configuration with different `node_id` and `advertise_address`.

#### 3. Initialize Cluster

On Node 1:

```bash
# Initialize cluster
nanograph-admin cluster init --config /etc/nanograph/server.toml

# Start node
sudo systemctl start nanograph
```

On Node 2 and Node 3:

```bash
# Join cluster
nanograph-admin cluster join --node-id node2 --cluster-address 10.0.1.10:7437

# Start node
sudo systemctl start nanograph
```

#### 4. Verify Cluster

```bash
# Check cluster status
nanograph-admin cluster status

# Expected output:
# Cluster: healthy
# Nodes: 3/3 online
# Shards: 16/16 healthy
# Leader: node1
```

### Kubernetes Deployment

#### StatefulSet

```yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: nanograph
spec:
  serviceName: nanograph
  replicas: 3
  selector:
    matchLabels:
      app: nanograph
  template:
    metadata:
      labels:
        app: nanograph
    spec:
      containers:
      - name: nanograph
        image: nanograph/server:latest
        ports:
        - containerPort: 7437
          name: client
        - containerPort: 7438
          name: cluster
        env:
        - name: NODE_ID
          valueFrom:
            fieldRef:
              fieldPath: metadata.name
        - name: CLUSTER_NODES
          value: "nanograph-0.nanograph:7437,nanograph-1.nanograph:7437,nanograph-2.nanograph:7437"
        volumeMounts:
        - name: data
          mountPath: /var/lib/nanograph
        resources:
          requests:
            memory: "4Gi"
            cpu: "2"
          limits:
            memory: "8Gi"
            cpu: "4"
  volumeClaimTemplates:
  - metadata:
      name: data
    spec:
      accessModes: ["ReadWriteOnce"]
      storageClassName: fast-ssd
      resources:
        requests:
          storage: 100Gi
```

#### Service

```yaml
apiVersion: v1
kind: Service
metadata:
  name: nanograph
spec:
  clusterIP: None
  selector:
    app: nanograph
  ports:
  - port: 7437
    name: client
  - port: 7438
    name: cluster
---
apiVersion: v1
kind: Service
metadata:
  name: nanograph-client
spec:
  type: LoadBalancer
  selector:
    app: nanograph
  ports:
  - port: 7437
    targetPort: 7437
```

Deploy:

```bash
kubectl apply -f nanograph-statefulset.yaml
kubectl apply -f nanograph-service.yaml
```

### Cluster Operations

#### Adding Nodes

```bash
# On new node
nanograph-admin cluster join \
  --node-id node4 \
  --cluster-address 10.0.1.10:7437

# Verify
nanograph-admin cluster status
```

#### Removing Nodes

```bash
# Graceful removal
nanograph-admin cluster remove --node-id node4

# Force removal (if node is down)
nanograph-admin cluster remove --node-id node4 --force
```

#### Rebalancing Shards

```bash
# Trigger rebalancing
nanograph-admin cluster rebalance

# Check rebalancing progress
nanograph-admin cluster rebalance-status
```

### Resource Requirements (Per Node)

**Minimum:**
- RAM: 4 GB
- Disk: 50 GB SSD
- CPU: 4 cores
- Network: 1 Gbps

**Recommended:**
- RAM: 16-32 GB
- Disk: 500 GB NVMe SSD
- CPU: 8-16 cores
- Network: 10 Gbps

---

## Configuration

### Configuration File Format

Nanograph uses TOML for configuration. Configuration can be:

1. **File-based:** `/etc/nanograph/server.toml`
2. **Environment variables:** `NANOGRAPH_*`
3. **Command-line flags:** `--option value`

Priority: CLI flags > Environment variables > Config file > Defaults

### Complete Configuration Reference

```toml
[server]
node_id = "node1"                    # Unique node identifier
bind_address = "0.0.0.0:7437"        # Listen address
advertise_address = "10.0.1.10:7437" # Address advertised to clients
max_connections = 10000              # Maximum concurrent connections
request_timeout_ms = 30000           # Request timeout
graceful_shutdown_timeout_s = 30     # Shutdown timeout

[cluster]
enabled = false                      # Enable cluster mode
initial_nodes = []                   # Initial cluster nodes
replication_factor = 3               # Number of replicas per shard
num_shards = 16                      # Number of shards
shard_placement_strategy = "hash"    # Placement strategy

[raft]
election_timeout_ms = 1000           # Election timeout
heartbeat_interval_ms = 100          # Heartbeat interval
snapshot_interval = 10000            # Snapshot interval (operations)
max_log_size_mb = 100                # Maximum log size before snapshot

[storage]
data_dir = "/var/lib/nanograph/data" # Data directory
wal_dir = "/var/lib/nanograph/wal"   # WAL directory
max_file_size_mb = 64                # Maximum file size
sync_writes = true                   # Fsync on writes

[memory]
cache_size_mb = 2048                 # Total cache size
memtable_size_mb = 256               # Memtable size
block_cache_mb = 1024                # Block cache size
write_buffer_size = 4194304          # Write buffer size (bytes)

[compaction]
style = "leveled"                    # Compaction style (leveled/tiered)
max_background_jobs = 4              # Background compaction threads
level_size_multiplier = 10           # Size multiplier between levels
max_levels = 7                       # Maximum number of levels

[performance]
read_threads = 8                     # Read thread pool size
write_threads = 4                    # Write thread pool size
compaction_threads = 4               # Compaction thread pool size
io_threads = 4                       # I/O thread pool size

[security]
tls_enabled = false                  # Enable TLS
tls_cert_path = ""                   # TLS certificate path
tls_key_path = ""                    # TLS key path
tls_ca_path = ""                     # TLS CA path (for mTLS)
require_auth = false                 # Require authentication
auth_token_secret = ""               # JWT secret for tokens

[logging]
level = "info"                       # Log level (trace/debug/info/warn/error)
output = "stdout"                    # Log output (stdout/file path)
format = "json"                      # Log format (json/text)
max_file_size_mb = 100               # Max log file size
max_backups = 10                     # Number of log backups

[metrics]
enabled = true                       # Enable metrics
bind_address = "0.0.0.0:9090"        # Metrics endpoint
format = "prometheus"                # Metrics format

[tracing]
enabled = false                      # Enable distributed tracing
endpoint = ""                        # Tracing endpoint (Jaeger/Zipkin)
sample_rate = 0.1                    # Sampling rate (0.0-1.0)
```

### Environment Variables

```bash
# Server
export NANOGRAPH_NODE_ID=node1
export NANOGRAPH_BIND_ADDRESS=0.0.0.0:7437

# Storage
export NANOGRAPH_DATA_DIR=/var/lib/nanograph/data
export NANOGRAPH_WAL_DIR=/var/lib/nanograph/wal

# Memory
export NANOGRAPH_CACHE_SIZE_MB=2048

# Security
export NANOGRAPH_TLS_ENABLED=true
export NANOGRAPH_TLS_CERT_PATH=/etc/nanograph/certs/server.crt
export NANOGRAPH_TLS_KEY_PATH=/etc/nanograph/certs/server.key
```

---

## Security

### TLS Configuration

#### Generate Certificates

```bash
# Generate CA
openssl genrsa -out ca.key 4096
openssl req -new -x509 -days 3650 -key ca.key -out ca.crt

# Generate server certificate
openssl genrsa -out server.key 4096
openssl req -new -key server.key -out server.csr
openssl x509 -req -days 365 -in server.csr -CA ca.crt -CAkey ca.key -set_serial 01 -out server.crt

# Install certificates
sudo mkdir -p /etc/nanograph/certs
sudo cp ca.crt server.crt server.key /etc/nanograph/certs/
sudo chmod 600 /etc/nanograph/certs/server.key
```

#### Enable TLS

```toml
[security]
tls_enabled = true
tls_cert_path = "/etc/nanograph/certs/server.crt"
tls_key_path = "/etc/nanograph/certs/server.key"
tls_ca_path = "/etc/nanograph/certs/ca.crt"  # For mTLS
```

### Authentication

#### Token-Based Authentication

```bash
# Generate auth token
nanograph-admin auth create-token --user admin --role admin

# Use token in client
export NANOGRAPH_AUTH_TOKEN=<token>
```

#### Role-Based Access Control

```toml
# Define roles
[[security.roles]]
name = "admin"
permissions = ["read", "write", "admin"]

[[security.roles]]
name = "readonly"
permissions = ["read"]

[[security.roles]]
name = "readwrite"
permissions = ["read", "write"]
```

### Network Security

#### Firewall Rules

```bash
# Allow client connections
sudo ufw allow 7437/tcp

# Allow cluster communication (internal only)
sudo ufw allow from 10.0.1.0/24 to any port 7438

# Allow metrics (internal only)
sudo ufw allow from 10.0.1.0/24 to any port 9090
```

#### Network Isolation

- Use private networks for cluster communication
- Expose only client port (7437) to public
- Use VPN or bastion hosts for admin access
- Enable TLS for all connections

---

## Monitoring

### Metrics

Nanograph exposes Prometheus-compatible metrics on `/metrics` endpoint.

#### Key Metrics

**Performance:**
- `nanograph_read_latency_seconds` - Read latency histogram
- `nanograph_write_latency_seconds` - Write latency histogram
- `nanograph_operations_total` - Total operations counter
- `nanograph_operations_errors_total` - Error counter

**Storage:**
- `nanograph_storage_size_bytes` - Total storage size
- `nanograph_wal_size_bytes` - WAL size
- `nanograph_memtable_size_bytes` - Memtable size
- `nanograph_compaction_duration_seconds` - Compaction duration

**Cluster:**
- `nanograph_cluster_nodes_total` - Total nodes
- `nanograph_cluster_nodes_healthy` - Healthy nodes
- `nanograph_cluster_shards_total` - Total shards
- `nanograph_cluster_shards_healthy` - Healthy shards

**System:**
- `nanograph_memory_usage_bytes` - Memory usage
- `nanograph_cpu_usage_percent` - CPU usage
- `nanograph_disk_usage_bytes` - Disk usage
- `nanograph_network_bytes_total` - Network traffic

#### Prometheus Configuration

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'nanograph'
    static_configs:
      - targets:
        - 'node1:9090'
        - 'node2:9090'
        - 'node3:9090'
    scrape_interval: 15s
```

### Grafana Dashboards

Import pre-built dashboards:

```bash
# Download dashboard
wget https://github.com/nanograph/nanograph/raw/main/monitoring/grafana-dashboard.json

# Import to Grafana
curl -X POST http://grafana:3000/api/dashboards/db \
  -H "Content-Type: application/json" \
  -d @grafana-dashboard.json
```

### Alerting

#### Prometheus Alerts

```yaml
# alerts.yml
groups:
  - name: nanograph
    rules:
      - alert: HighErrorRate
        expr: rate(nanograph_operations_errors_total[5m]) > 0.01
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High error rate detected"
      
      - alert: NodeDown
        expr: up{job="nanograph"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Nanograph node is down"
      
      - alert: HighLatency
        expr: histogram_quantile(0.99, nanograph_read_latency_seconds) > 0.1
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High read latency detected"
```

### Logging

#### Log Aggregation

Use ELK stack or similar:

```yaml
# filebeat.yml
filebeat.inputs:
  - type: log
    enabled: true
    paths:
      - /var/log/nanograph/*.log
    json.keys_under_root: true
    json.add_error_key: true

output.elasticsearch:
  hosts: ["elasticsearch:9200"]
  index: "nanograph-%{+yyyy.MM.dd}"
```

---

## Backup and Recovery

### Backup Strategies

#### 1. Snapshot Backups

```bash
# Create snapshot
nanograph-admin backup create --output /backups/snapshot-$(date +%Y%m%d-%H%M%S).tar.gz

# Automated daily backups
0 2 * * * /usr/local/bin/nanograph-admin backup create --output /backups/daily-$(date +\%Y\%m\%d).tar.gz
```

#### 2. Incremental Backups

```bash
# Create incremental backup
nanograph-admin backup create --incremental --base /backups/full-backup.tar.gz --output /backups/incremental-$(date +%Y%m%d).tar.gz
```

#### 3. Continuous Backup (WAL Archiving)

```toml
[backup]
wal_archive_enabled = true
wal_archive_dir = "/backups/wal-archive"
wal_archive_retention_days = 7
```

### Restore Procedures

#### Full Restore

```bash
# Stop server
sudo systemctl stop nanograph

# Clear data directory
sudo rm -rf /var/lib/nanograph/data/*

# Restore from backup
sudo tar -xzf /backups/snapshot-20260107.tar.gz -C /var/lib/nanograph/data/

# Fix permissions
sudo chown -R nanograph:nanograph /var/lib/nanograph/data

# Start server
sudo systemctl start nanograph
```

#### Point-in-Time Recovery

```bash
# Restore base backup
sudo tar -xzf /backups/full-backup.tar.gz -C /var/lib/nanograph/data/

# Replay WAL files
nanograph-admin recovery replay-wal \
  --wal-dir /backups/wal-archive \
  --target-time "2026-01-07 12:00:00"

# Start server
sudo systemctl start nanograph
```

### Backup Best Practices

1. **Frequency:** Daily full backups, hourly incrementals
2. **Retention:** Keep 7 daily, 4 weekly, 12 monthly backups
3. **Verification:** Test restore procedures regularly
4. **Off-site:** Store backups in different location/region
5. **Encryption:** Encrypt backups at rest and in transit
6. **Monitoring:** Alert on backup failures

---

## Performance Tuning

### Storage Optimization

#### SSD Configuration

```bash
# Enable TRIM
sudo systemctl enable fstrim.timer

# Set I/O scheduler
echo "none" | sudo tee /sys/block/nvme0n1/queue/scheduler

# Disable access time updates
# Add to /etc/fstab: noatime,nodiratime
```

#### File System

Recommended: XFS or ext4 with:
- Large block size (4KB)
- Journal on separate device (if possible)
- Disabled access time updates

### Memory Tuning

```toml
[memory]
# Rule of thumb: 50% of available RAM
cache_size_mb = 8192

# Memtable: 5-10% of cache
memtable_size_mb = 512

# Block cache: remaining cache
block_cache_mb = 7680
```

### Compaction Tuning

```toml
[compaction]
# Leveled compaction for read-heavy workloads
style = "leveled"
level_size_multiplier = 10
max_levels = 7

# Tiered compaction for write-heavy workloads
# style = "tiered"
# max_background_jobs = 8
```

### Network Tuning

```bash
# Increase TCP buffer sizes
sudo sysctl -w net.core.rmem_max=16777216
sudo sysctl -w net.core.wmem_max=16777216
sudo sysctl -w net.ipv4.tcp_rmem="4096 87380 16777216"
sudo sysctl -w net.ipv4.tcp_wmem="4096 65536 16777216"

# Enable TCP fast open
sudo sysctl -w net.ipv4.tcp_fastopen=3
```

### OS Tuning

```bash
# Increase file descriptor limit
ulimit -n 65536

# Disable transparent huge pages
echo never | sudo tee /sys/kernel/mm/transparent_hugepage/enabled

# Increase max map count
sudo sysctl -w vm.max_map_count=262144
```

---

## Troubleshooting

### Common Issues

#### 1. High Latency

**Symptoms:** Slow read/write operations

**Diagnosis:**
```bash
# Check metrics
curl http://localhost:9090/metrics | grep latency

# Check compaction status
nanograph-admin status compaction

# Check disk I/O
iostat -x 1
```

**Solutions:**
- Increase cache size
- Add more compaction threads
- Upgrade to faster storage
- Check for disk contention

#### 2. Out of Memory

**Symptoms:** OOM errors, crashes

**Diagnosis:**
```bash
# Check memory usage
nanograph-admin status memory

# Check system memory
free -h
```

**Solutions:**
- Reduce cache size
- Reduce memtable size
- Add more RAM
- Enable swap (not recommended for production)

#### 3. Cluster Split-Brain

**Symptoms:** Multiple leaders, inconsistent data

**Diagnosis:**
```bash
# Check cluster status
nanograph-admin cluster status

# Check Raft state
nanograph-admin cluster raft-status
```

**Solutions:**
- Ensure odd number of nodes (3, 5, 7)
- Check network connectivity
- Verify time synchronization (NTP)
- Restart affected nodes

#### 4. Slow Compaction

**Symptoms:** Growing disk usage, degraded performance

**Diagnosis:**
```bash
# Check compaction progress
nanograph-admin status compaction

# Check disk I/O
iostat -x 1
```

**Solutions:**
- Increase compaction threads
- Upgrade storage
- Adjust compaction strategy
- Schedule compaction during off-peak hours

### Debug Mode

Enable debug logging:

```toml
[logging]
level = "debug"
```

Or temporarily:

```bash
nanograph-admin set-log-level debug
```

### Support Information

When reporting issues, include:

1. Nanograph version
2. Configuration file
3. Log files (last 1000 lines)
4. Metrics snapshot
5. System information (OS, RAM, CPU, disk)
6. Steps to reproduce

---

## Production Checklist

### Pre-Deployment

- [ ] Hardware meets requirements
- [ ] OS is updated and hardened
- [ ] Firewall rules configured
- [ ] TLS certificates generated
- [ ] Configuration reviewed
- [ ] Backup strategy defined
- [ ] Monitoring configured
- [ ] Alerting configured
- [ ] Documentation reviewed
- [ ] Team trained

### Deployment

- [ ] Install Nanograph
- [ ] Configure server
- [ ] Start service
- [ ] Verify connectivity
- [ ] Run smoke tests
- [ ] Check metrics
- [ ] Verify backups
- [ ] Document deployment

### Post-Deployment

- [ ] Monitor for 24 hours
- [ ] Review logs
- [ ] Check performance
- [ ] Verify backups
- [ ] Test failover (cluster mode)
- [ ] Update documentation
- [ ] Schedule maintenance windows

### Ongoing Operations

- [ ] Daily: Check metrics and alerts
- [ ] Weekly: Review logs and performance
- [ ] Monthly: Test backup restore
- [ ] Quarterly: Review capacity and scaling
- [ ] Annually: Security audit

---

## Additional Resources

- [Architecture Documentation](ARCHITECTURE_APPENDICES.md)
- [API Reference](ADR/ADR-0025-Core-API-Specifications.md)
- [Performance Benchmarks](ADR/ADR-0027-Performance-Benchmarks-and-Testing.md)
- [Contributing Guide](../CONTRIBUTING.md)
- [Glossary](GLOSSARY.md)

---

## Support

For deployment assistance:

- **Documentation:** https://docs.nanograph.io
- **Community:** https://github.com/nanograph/nanograph/discussions
- **Issues:** https://github.com/nanograph/nanograph/issues
- **Email:** support@nanograph.io

---

*Last Updated: 2026-01-07*