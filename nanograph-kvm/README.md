# Nanograph KVM Server

A simple REST API server wrapper around the nanograph-kvm key-value database.

## Features

- Simple REST API for key-value operations
- GET, PUT, and DELETE endpoints
- Health check endpoint
- Configurable host and port
- Structured logging with tracing
- Command-line and environment variable configuration

## Installation

```bash
cargo build --release --bin nanograph-kvm-server
```

## Usage

### Starting the Server

```bash
# Default configuration (127.0.0.1:8080)
cargo run --bin nanograph-kvm-server

# Custom host and port
cargo run --bin nanograph-kvm-server -- --host 0.0.0.0 --port 3000

# With debug logging
cargo run --bin nanograph-kvm-server -- --log-level debug
```

### Environment Variables

You can also configure the server using environment variables:

```bash
export KVM_HOST=0.0.0.0
export KVM_PORT=3000
export KVM_LOG_LEVEL=debug
cargo run --bin nanograph-kvm-server
```

## API Endpoints

### Health Check

```bash
GET /health
```

Returns `200 OK` if the server is running.

### Store a Key-Value Pair

```bash
PUT /api/v1/{tenant_id}/{container_id}/{table_id}
Content-Type: application/json

{
  "key": "user:123",
  "value": "John Doe"
}
```

### Retrieve a Value

```bash
GET /api/v1/{tenant_id}/{container_id}/{table_id}/{key}
```

Returns:
```json
{
  "key": "user:123",
  "value": "John Doe"
}
```

Or if the key doesn't exist:
```json
{
  "key": "user:123",
  "value": null
}
```

### Delete a Key-Value Pair

```bash
DELETE /api/v1/{tenant_id}/{container_id}/{table_id}/{key}
```

Returns:
```json
{
  "key": "user:123",
  "deleted": true
}
```

## Examples

### Using curl

```bash
# Store a value
curl -X PUT http://localhost:8080/api/v1/tenant1/container1/table1 \
  -H "Content-Type: application/json" \
  -d '{"key": "user:123", "value": "John Doe"}'

# Retrieve a value
curl http://localhost:8080/api/v1/tenant1/container1/table1/user:123

# Delete a value
curl -X DELETE http://localhost:8080/api/v1/tenant1/container1/table1/user:123
```

### Using httpie

```bash
# Store a value
http PUT localhost:8080/api/v1/tenant1/container1/table1 \
  key=user:123 value="John Doe"

# Retrieve a value
http GET localhost:8080/api/v1/tenant1/container1/table1/user:123

# Delete a value
http DELETE localhost:8080/api/v1/tenant1/container1/table1/user:123
```

## Command-Line Options

```
Options:
  -H, --host <HOST>              Host address to bind to [default: 127.0.0.1] [env: KVM_HOST]
  -p, --port <PORT>              Port to listen on [default: 8080] [env: KVM_PORT]
  -l, --log-level <LOG_LEVEL>    Log level (trace, debug, info, warn, error) [default: info] [env: KVM_LOG_LEVEL]
  -h, --help                     Print help
  -V, --version                  Print version
```

## Architecture

The server is built using:
- **axum**: Modern web framework for Rust
- **tokio**: Async runtime (required for async operations)
- **tower-http**: HTTP middleware (tracing, CORS)
- **clap**: Command-line argument parsing
- **tracing**: Structured logging

The server creates a standalone instance of `KeyValueDatabaseManager` and exposes its operations through REST endpoints.

### Async/Await Architecture

The entire `nanograph-kvm` library uses async/await patterns:
- All database operations are asynchronous
- Uses `tokio::sync::RwLock` for thread-safe concurrent access
- All handlers are async functions that await database operations
- Requires a tokio runtime to execute

**Example Usage:**
```rust
use nanograph_kvm::KeyValueDatabaseManager;

#[tokio::main]
async fn main() {
    let manager = KeyValueDatabaseManager::new(config);
    
    // All operations are async and must be awaited
    let value = manager.get(&principal, tenant_id, container_id, table_id, key).await?;
    manager.put(&principal, tenant_id, container_id, table_id, key, value).await?;
    manager.delete(&principal, tenant_id, container_id, table_id, key).await?;
}
```

## License

Licensed under the Apache License, Version 2.0.