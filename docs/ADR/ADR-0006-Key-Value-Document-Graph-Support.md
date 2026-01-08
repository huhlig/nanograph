---
parent: ADR
nav_order: 0006
title: Key-Value, Document, and Graph Support
status: accepted
date: 2026-01-05
deciders: Hans W. Uhlig
---

# ADR-0006: Key-Value, Document, and Graph Support

## Status

Accepted

## Context

Nanograph aims to be a multi-model database supporting key-value, document, and graph data models. Traditional approaches either:

1. **Implement separate storage engines** for each model, leading to code duplication, inconsistent behavior, and operational complexity
2. **Force a single model** (e.g., document-only), limiting flexibility and requiring awkward workarounds for other use cases
3. **Use a generic abstraction** that performs poorly for all models

We need an approach that provides native support for all three models while maintaining:
- **Consistent ACID guarantees** across models
- **Efficient storage** without excessive overhead
- **Simple implementation** to reduce maintenance burden
- **Predictable performance** characteristics

## Decision

Adopt a **KV-first layered architecture** where:

1. **Key-Value is the foundational storage primitive**
2. **Document and Graph models compile to KV operations**
3. **All models share the same storage engine, WAL, and transaction system**
4. **Higher-level abstractions provide model-specific APIs**

This approach treats KV as the "assembly language" of the database, with Document and Graph as higher-level "languages" that compile down to KV operations.

## Decision Drivers

* **Simplicity** - Single storage engine to implement and maintain
* **Consistency** - Uniform ACID guarantees across all models
* **Performance** - Direct KV access for hot paths, no abstraction overhead
* **Flexibility** - Easy to add new models in the future
* **Debuggability** - All operations ultimately visible as KV operations
* **Testing** - Single storage layer to test thoroughly


### System Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Application Layer                            │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                    ┌───────────────┼───────────────┐
                    ▼               ▼               ▼
        ┌───────────────┐  ┌───────────────┐  ┌───────────────┐
        │   KV API      │  │ Document API  │  │   Graph API   │
        │               │  │               │  │               │
        │ • put/get     │  │ • insert      │  │ • addNode     │
        │ • delete      │  │ • update      │  │ • addEdge     │
        │ • scan        │  │ • query       │  │ • traverse    │
        │ • batch       │  │ • index       │  │ • neighbors   │
        └───────────────┘  └───────────────┘  └───────────────┘
                │                  │                  │
                │                  ▼                  ▼
                │          ┌───────────────┐  ┌───────────────┐
                │          │ Document      │  │ Graph         │
                │          │ Compiler      │  │ Compiler      │
                │          │               │  │               │
                │          │ JSON → KV     │  │ Nodes/Edges   │
                │          │ Indexes       │  │ → KV          │
                │          └───────────────┘  └───────────────┘
                │                  │                  │
                └──────────────────┴──────────────────┘
                                    │
                                    ▼
        ┌─────────────────────────────────────────────────────┐
        │            Transaction Coordinator                   │
        │  • MVCC version management                          │
        │  • Snapshot isolation                               │
        │  • Conflict detection                               │
        └─────────────────────────────────────────────────────┘
                                    │
                                    ▼
        ┌─────────────────────────────────────────────────────┐
        │              Core KV Storage Engine                  │
        │  • Memtable (in-memory buffer)                      │
        │  • SSTable (sorted string table)                    │
        │  • LSM tree / B+ tree                               │
        │  • Compaction                                       │
        └─────────────────────────────────────────────────────┘
                                    │
                    ┌───────────────┴───────────────┐
                    ▼                               ▼
        ┌───────────────────┐           ┌───────────────────┐
        │  Write-Ahead Log  │           │  Index Subsystem  │
        │  • Durability     │           │  • B-tree indexes │
        │  • Recovery       │           │  • Vector indexes │
        │  • Replication    │           │  • Text indexes   │
        └───────────────────┘           └───────────────────┘
                    │
                    ▼
        ┌─────────────────────────────────────────────────────┐
        │          Virtual File System (VFS)                   │
        │  • OS filesystem                                     │
        │  • In-memory (testing)                              │
        │  • Cloud storage (future)                           │
        └─────────────────────────────────────────────────────┘
```

## Design

### 1. Key-Value Model (Foundation)

The KV model is the lowest-level abstraction, directly mapping to the storage engine.

#### Key Structure

```rust
struct Key {
    table_id: u32,      // Logical table identifier
    key_data: Vec<u8>,  // Opaque key bytes
}
```

#### Operations

```rust
trait KvStore {
    fn get(&self, table: TableId, key: &[u8]) -> Result<Option<Vec<u8>>>;
    fn put(&self, table: TableId, key: &[u8], value: &[u8]) -> Result<()>;
    fn delete(&self, table: TableId, key: &[u8]) -> Result<bool>;
    fn scan(&self, table: TableId, range: Range) -> Result<Iterator>;
}
```

#### Characteristics

- **Byte-oriented** - Keys and values are opaque byte arrays
- **Ordered** - Keys are sorted lexicographically
- **Atomic** - Single-key operations are atomic
- **Durable** - Writes are persisted via WAL

### 2. Document Model (Layer 1)

Documents are stored as KV entries with additional indexing structures.

#### Storage Layout

```
Primary Storage:
  Key:   [table_id][doc_id]
  Value: [document_json]

Secondary Indexes:
  Key:   [table_id][index_id][field_value][doc_id]
  Value: [empty or doc_id]
```

#### Document ID Generation

```rust
fn generate_doc_id() -> DocumentId {
    // Timestamp (48 bits) + Random (80 bits) = 128 bits
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    
    let random = rand::random::<u128>() & 0xFFFF_FFFF_FFFF_FFFF_FFFF;
    
    DocumentId::from_parts(timestamp, random)
}
```

#### Field Indexing

For a document with indexed field `email`:

```json
{
  "id": "doc123",
  "email": "user@example.com",
  "name": "Alice"
}
```

Storage entries:
```
Primary:
  [table:users][doc123] -> {"id":"doc123","email":"user@example.com","name":"Alice"}

Index:
  [table:users][index:email]["user@example.com"][doc123] -> []
```

#### Query Translation

Document query:
```rust
collection.query(Filter::eq("email", "user@example.com"))
```

Translates to KV operations:
```rust
// Use index scan
let index_key_prefix = encode_index_key(table_id, index_id, "user@example.com");
let doc_ids = kv_store.scan(table_id, prefix_range(index_key_prefix));

// Fetch documents
for doc_id in doc_ids {
    let doc_key = encode_doc_key(table_id, doc_id);
    let doc_data = kv_store.get(table_id, &doc_key)?;
    // Deserialize and return
}
```

#### Partial Updates

Partial document updates use read-modify-write:

```rust
fn patch_document(doc_id: DocumentId, patch: JsonPatch) -> Result<()> {
    let tx = begin_transaction()?;
    
    // Read current document
    let key = encode_doc_key(table_id, doc_id);
    let current = tx.get(table_id, &key)?;
    
    // Apply patch
    let mut doc: serde_json::Value = serde_json::from_slice(&current)?;
    patch.apply(&mut doc)?;
    
    // Write back
    let new_data = serde_json::to_vec(&doc)?;
    tx.put(table_id, &key, &new_data)?;
    
    // Update indexes if needed
    update_indexes(&tx, table_id, doc_id, &doc)?;
    
    tx.commit()
}
```

### 3. Graph Model (Layer 1)

Graphs are stored using adjacency lists and edge records.

#### Storage Layout

```
Nodes:
  Key:   [table_id][N][node_id]
  Value: [node_properties_json]

Outgoing Edges:
  Key:   [table_id][E][from_node][edge_type][to_node][edge_id]
  Value: [edge_properties_json]

Incoming Edges (optional, for bidirectional traversal):
  Key:   [table_id][I][to_node][edge_type][from_node][edge_id]
  Value: [edge_id_reference]

Edge Metadata:
  Key:   [table_id][M][edge_id]
  Value: [from_node][to_node][edge_type][properties]
```

#### Key Encoding

```rust
fn encode_node_key(table_id: u32, node_id: u64) -> Vec<u8> {
    let mut key = Vec::with_capacity(13);
    key.extend_from_slice(&table_id.to_be_bytes());
    key.push(b'N'); // Node marker
    key.extend_from_slice(&node_id.to_be_bytes());
    key
}

fn encode_edge_key(
    table_id: u32,
    from: u64,
    edge_type: &str,
    to: u64,
    edge_id: u64
) -> Vec<u8> {
    let mut key = Vec::new();
    key.extend_from_slice(&table_id.to_be_bytes());
    key.push(b'E'); // Edge marker
    key.extend_from_slice(&from.to_be_bytes());
    key.extend_from_slice(edge_type.as_bytes());
    key.push(0); // Null terminator
    key.extend_from_slice(&to.to_be_bytes());
    key.extend_from_slice(&edge_id.to_be_bytes());
    key
}
```

#### Traversal Operations

**Get neighbors:**
```rust
fn get_neighbors(node_id: u64, direction: Direction) -> Result<Vec<u64>> {
    let prefix = match direction {
        Direction::Outgoing => encode_outgoing_prefix(table_id, node_id),
        Direction::Incoming => encode_incoming_prefix(table_id, node_id),
        Direction::Both => {
            // Scan both and merge
        }
    };
    
    let edges = kv_store.scan(table_id, prefix_range(prefix))?;
    
    // Extract neighbor IDs from keys
    edges.map(|(key, _)| extract_neighbor_id(&key)).collect()
}
```

**Multi-hop traversal:**
```rust
fn traverse(start: u64, max_depth: usize) -> Result<Vec<u64>> {
    let mut visited = HashSet::new();
    let mut current_level = vec![start];
    let mut all_nodes = vec![start];
    
    for _ in 0..max_depth {
        let mut next_level = Vec::new();
        
        for node in current_level {
            if visited.contains(&node) {
                continue;
            }
            visited.insert(node);
            
            let neighbors = get_neighbors(node, Direction::Outgoing)?;
            next_level.extend(neighbors);
            all_nodes.extend(&next_level);
        }
        
        if next_level.is_empty() {
            break;
        }
        
        current_level = next_level;
    }
    
    Ok(all_nodes)
}
```

#### Edge Properties

Edges can have properties stored in their values:

```rust
struct Edge {
    from: NodeId,
    to: NodeId,
    edge_type: String,
    properties: HashMap<String, Value>,
    created_at: Timestamp,
}

// Store edge with properties
fn create_edge(from: u64, to: u64, edge_type: &str, props: Properties) -> Result<EdgeId> {
    let edge_id = generate_edge_id();
    let edge = Edge {
        from,
        to,
        edge_type: edge_type.to_string(),
        properties: props,
        created_at: now(),
    };
    
    let tx = begin_transaction()?;
    
    // Store in outgoing adjacency list
    let out_key = encode_edge_key(table_id, from, edge_type, to, edge_id);
    tx.put(table_id, &out_key, &serialize(&edge)?)?;
    
    // Store in incoming adjacency list (for bidirectional)
    let in_key = encode_incoming_edge_key(table_id, to, edge_type, from, edge_id);
    tx.put(table_id, &in_key, &edge_id.to_be_bytes())?;
    
    // Store edge metadata
    let meta_key = encode_edge_meta_key(table_id, edge_id);
    tx.put(table_id, &meta_key, &serialize(&edge)?)?;
    
    tx.commit()?;
    Ok(edge_id)
}
```

### 4. Cross-Model Operations

The KV foundation enables operations that span multiple models:

```rust
// Find documents related to a graph node
fn find_related_documents(node_id: u64) -> Result<Vec<Document>> {
    // Get neighbors from graph
    let neighbors = graph.get_neighbors(node_id, Direction::Both)?;
    
    // Fetch corresponding documents
    let mut docs = Vec::new();
    for neighbor in neighbors {
        let doc_id = node_to_doc_id(neighbor);
        if let Some(doc) = document_store.get(doc_id)? {
            docs.push(doc);
        }
    }
    
    Ok(docs)
}
```

### 5. Transaction Semantics

All models share the same transaction system:

```rust
let tx = db.begin_transaction()?;

// Mix operations across models
tx.kv_put(table1, key1, value1)?;
tx.doc_insert(collection, document)?;
tx.graph_create_edge(from, to, edge_type, props)?;

// All succeed or all fail
tx.commit()?;
```

### 6. Performance Considerations

#### Hot Path Optimization

For performance-critical code, bypass higher-level abstractions:

```rust
// Instead of:
let doc = document_store.get(doc_id)?;

// Use direct KV access:
let key = encode_doc_key(table_id, doc_id);
let raw_data = kv_store.get(table_id, &key)?;
let doc = deserialize_document(&raw_data)?;
```

#### Batch Operations

Leverage KV batch operations for efficiency:

```rust
fn batch_insert_documents(docs: Vec<Document>) -> Result<()> {
    let tx = begin_transaction()?;
    
    let mut kv_pairs = Vec::new();
    for doc in docs {
        let key = encode_doc_key(table_id, doc.id);
        let value = serialize_document(&doc)?;
        kv_pairs.push((key, value));
    }
    
    tx.batch_put(table_id, &kv_pairs)?;
    tx.commit()
}
```

#### Index Maintenance

Secondary indexes are maintained transactionally:

```rust
fn update_document_with_indexes(doc_id: DocumentId, new_doc: Document) -> Result<()> {
    let tx = begin_transaction()?;
    
    // Get old document for index cleanup
    let old_doc = get_document(&tx, doc_id)?;
    
    // Update primary storage
    let key = encode_doc_key(table_id, doc_id);
    tx.put(table_id, &key, &serialize(&new_doc)?)?;
    
    // Update indexes
    for index in get_indexes(table_id)? {
        // Remove old index entries
        let old_value = extract_field(&old_doc, &index.field);
        let old_index_key = encode_index_key(table_id, index.id, &old_value, doc_id);
        tx.delete(table_id, &old_index_key)?;
        
        // Add new index entries
        let new_value = extract_field(&new_doc, &index.field);
        let new_index_key = encode_index_key(table_id, index.id, &new_value, doc_id);
        tx.put(table_id, &new_index_key, &[])?;
    }
    
    tx.commit()
}
```

## Consequences

### Positive

* **Unified storage semantics** - Single storage engine, WAL, and transaction system
* **Reduced code duplication** - No need for separate engines per model
* **Consistent ACID guarantees** - All models benefit from same transaction system
* **Flexible data modeling** - Easy to mix models or add new ones
* **Debuggability** - All operations visible at KV level
* **Performance predictability** - Well-understood KV performance characteristics
* **Simpler testing** - Test storage engine once, models inherit correctness
* **Efficient storage** - No redundant data structures

### Negative

* **More complex higher-level logic** - Document and graph layers require careful encoding
* **Potential overhead** - Encoding/decoding adds CPU cost
* **Index maintenance complexity** - Secondary indexes require careful transaction handling
* **Graph traversal limitations** - Deep traversals may require many KV operations
* **Learning curve** - Developers must understand the layering

### Risks

* **Performance bottlenecks** - If encoding/decoding becomes expensive
* **Key space collisions** - Careful key encoding required to avoid conflicts
* **Index bloat** - Secondary indexes can grow large
* **Graph hot spots** - High-degree nodes may cause contention

## Alternatives Considered

### 1. Separate Storage Engines Per Model

**Rejected** - Would require maintaining multiple storage engines, WAL implementations, and transaction systems. Increases complexity and operational burden.

### 2. Document-First with Graph Extension

**Rejected** - Forces awkward graph representations and limits performance for graph operations.

### 3. Graph Database with Document Support

**Rejected** - Graph databases typically don't provide efficient document operations or secondary indexing.

### 4. Generic Abstraction Layer

**Rejected** - Tends to be inefficient for all models due to lowest-common-denominator design.

## Implementation Notes

### Phase 1: KV Foundation (Weeks 3-8)
- Implement core KV storage engine
- Add transaction support
- Create comprehensive test suite

### Phase 2: Document Model (Weeks 15-17)
- Implement document encoding/decoding
- Add secondary index support
- Create document API

### Phase 3: Graph Model (Weeks 18-20)
- Implement graph storage layout
- Add traversal algorithms
- Optimize for common patterns

## Related ADRs

* [ADR-0004: Storage File Formats](ADR-0004-Storage-File-Formats.md)
* [ADR-0008: Indexing Options](ADR-0008-Indexing-Options.md)
* [ADR-0012: Transaction Model and Isolation Levels](ADR-0012-Transaction-Model-and-Isolation-Levels.md)
* [ADR-0025: Core API Specifications](ADR-0025-Core-API-Specifications.md)

## References

* FoundationDB's layer concept
* RocksDB column families
* Graph database storage patterns
* Document database indexing strategies

---

**Next Steps:**
1. Implement KV storage engine
2. Define key encoding standards
3. Create document layer
4. Implement graph storage
5. Add comprehensive tests for each model
