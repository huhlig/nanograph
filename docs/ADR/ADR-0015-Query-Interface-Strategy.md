---
parent: ADR
nav_order: 0015
title: Query Interface Strategy
status: accepted
date: 2026-01-05
deciders: Hans W. Uhlig
---

# ADR-0015: Query Interface Strategy

## Status

Accepted

## Context

Nanograph supports multiple data models (KV, document, graph, vector) and must provide query interfaces that are:

1. **Type-safe** - Catch errors at compile time
2. **Composable** - Build complex queries from simple operations
3. **Embeddable** - Work well in application code
4. **Language-agnostic** - Support multiple programming languages
5. **Performant** - Enable optimization opportunities
6. **Flexible** - Support ad-hoc and programmatic queries

Traditional approaches have limitations:
- **SQL** - Not ideal for graph or vector operations
- **Cypher** - Graph-specific, doesn't handle documents well
- **MongoDB query language** - Document-focused, limited graph support
- **Custom DSL** - Requires learning new syntax, hard to extend

## Decision

Adopt a **layered query interface strategy**:

1. **Layer 1: Programmatic API** (Primary)
   - Strongly typed Rust API
   - Composable query builders
   - Direct integration with application code

2. **Layer 2: Query AST** (Internal)
   - Abstract syntax tree representation
   - Enables optimization and planning
   - Serializable for network transport

3. **Layer 3: Optional DSL** (Future)
   - JSON-based query language
   - SQL-like syntax for familiar operations

## Architecture

### Query Processing Pipeline

```
┌─────────────────────────────────────────────────────────────────┐
│                      Application Code                            │
└─────────────────────────────────────────────────────────────────┘
                                │
                ┌───────────────┼───────────────┐
                ▼               ▼               ▼
    ┌──────────────────┐ ┌──────────────┐ ┌──────────────┐
    │  KV Query API    │ │ Doc Query API│ │Graph Query API│
    │                  │ │              │ │               │
    │ db.scan()        │ │ db.find()    │ │ db.traverse() │
    │   .filter()      │ │   .where()   │ │   .from()     │
    │   .limit()       │ │   .project() │ │   .depth()    │
    └──────────────────┘ └──────────────┘ └──────────────┘
                │               │               │
                └───────────────┴───────────────┘
                                │
                                ▼
                    ┌───────────────────────┐
                    │   Query Builder       │
                    │  (Type-safe API)      │
                    └───────────────────────┘
                                │
                                ▼
                    ┌───────────────────────┐
                    │   Query AST           │
                    │  (Internal repr.)     │
                    │                       │
                    │  • Scan               │
                    │  • Filter             │
                    │  • Project            │
                    │  • Join               │
                    │  • Aggregate          │
                    └───────────────────────┘
                                │
                                ▼
                    ┌───────────────────────┐
                    │  Query Optimizer      │
                    │                       │
                    │  • Predicate pushdown │
                    │  • Index selection    │
                    │  • Join reordering    │
                    │  • Cost estimation    │
                    └───────────────────────┘
                                │
                                ▼
                    ┌───────────────────────┐
                    │  Physical Plan        │
                    │                       │
                    │  • Execution strategy │
                    │  • Operator pipeline  │
                    │  • Resource allocation│
                    └───────────────────────┘
                                │
                                ▼
                    ┌───────────────────────┐
                    │  Query Executor       │
                    │                       │
                    │  • Iterator-based     │
                    │  • Streaming results  │
                    │  • Parallel execution │
                    └───────────────────────┘
                                │
                ┌───────────────┼───────────────┐
                ▼               ▼               ▼
    ┌──────────────────┐ ┌──────────────┐ ┌──────────────┐
    │  Storage Engine  │ │    Indexes   │ │  Vector ANN  │
    └──────────────────┘ └──────────────┘ └──────────────┘
                                │
                                ▼
                    ┌───────────────────────┐
                    │   Result Stream       │
                    └───────────────────────┘
                                │
                                ▼
                    ┌───────────────────────┐
                    │   Application         │
                    └───────────────────────┘
```

### Query Operator Graph

```
Example: Find users in a city with similar interests

                    ┌─────────────┐
                    │   Scan      │
                    │  (users)    │
                    └─────────────┘
                          │
                          ▼
                    ┌─────────────┐
                    │   Filter    │
                    │ (city="SF") │
                    └─────────────┘
                          │
                          ▼
                    ┌─────────────┐
                    │   Project   │
                    │ (id, embed) │
                    └─────────────┘
                          │
                          ▼
                    ┌─────────────┐
                    │  Vector ANN │
                    │  (k=10)     │
                    └─────────────┘
                          │
                          ▼
                    ┌─────────────┐
                    │    Join     │
                    │  (user_id)  │
                    └─────────────┘
                          │
                          ▼
                    ┌─────────────┐
                    │   Project   │
                    │ (name, bio) │
                    └─────────────┘
                          │
                          ▼
                    ┌─────────────┐
                    │   Limit     │
                    │   (n=10)    │
                    └─────────────┘
```

### Hybrid Query Execution

```
Query: Find documents matching "database" with vector similarity

┌─────────────────────────────────────────────────────────────┐
│                    Query Coordinator                         │
└─────────────────────────────────────────────────────────────┘
                            │
            ┌───────────────┴───────────────┐
            ▼                               ▼
┌───────────────────────┐       ┌───────────────────────┐
│  Keyword Search       │       │  Vector Search        │
│                       │       │                       │
│  1. Text index scan   │       │  1. ANN index search  │
│  2. BM25 scoring      │       │  2. Distance calc     │
│  3. Top-K results     │       │  3. Top-K results     │
│                       │       │                       │
│  Results: [id, score] │       │  Results: [id, score] │
└───────────────────────┘       └───────────────────────┘
            │                               │
            └───────────────┬───────────────┘
                            ▼
                ┌───────────────────────┐
                │   Result Merger       │
                │                       │
                │  • Combine scores     │
                │  • Re-rank results    │
                │  • Apply weights      │
                └───────────────────────┘
                            │
                            ▼
                ┌───────────────────────┐
                │   Fetch Documents     │
                │  (from storage)       │
                └───────────────────────┘
                            │
                            ▼
                ┌───────────────────────┐
                │   Final Results       │
                └───────────────────────┘
```

### Graph Traversal Execution

```
Query: Find friends-of-friends within 2 hops

Start Node: User A
                            ┌─────────┐
                            │ User A  │
                            └─────────┘
                                 │
                    ┌────────────┼────────────┐
                    ▼            ▼            ▼
              ┌─────────┐  ┌─────────┐  ┌─────────┐
              │ User B  │  │ User C  │  │ User D  │  Hop 1
              └─────────┘  └─────────┘  └─────────┘
                    │            │            │
        ┌───────────┼────┐  ┌────┼────┐  ┌────┼───────┐
        ▼           ▼    ▼  ▼    ▼    ▼  ▼    ▼       ▼
    ┌─────┐   ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐  Hop 2
    │ E   │   │ F   │ │ G   │ │ H   │ │ I   │ │ J   │
    └─────┘   └─────┘ └─────┘ └─────┘ └─────┘ └─────┘

Execution Strategy:
1. Breadth-first traversal
2. Batch edge lookups per level
3. Deduplication across levels
4. Early termination on depth limit
5. Filter predicates at each hop
```

### Query Optimization Example

```
Original Query:
  Scan(users)
    → Filter(age > 25)
    → Filter(city = "SF")
    → Project(name, email)
    → Limit(10)

Optimized Query:
  IndexScan(users, city_idx, "SF")  ← Index selection
    → Filter(age > 25)               ← Predicate pushdown
    → Limit(10)                      ← Early termination
    → Project(name, email)           ← Late projection

Optimizations Applied:
1. Index selection: Use city index instead of full scan
2. Predicate pushdown: Apply filters early
3. Limit pushdown: Stop after 10 results
4. Late projection: Fetch only needed fields
```

   - Graph query extensions

This approach prioritizes embeddability and type safety while leaving room for declarative query languages in the future.

## Decision Drivers

* **Embeddability** - API-first design works well in embedded mode
* **Type safety** - Compile-time guarantees reduce errors
* **SDK generation** - Programmatic API translates well to other languages
* **Flexibility** - Easy to add DSL later without breaking existing code
* **Performance** - Direct API calls avoid parsing overhead
* **Incremental adoption** - Start simple, add complexity as needed

## Design

### 1. Programmatic Query API

#### KV Queries

```rust
// Simple get
let value = db.kv()
    .table("users")
    .get(b"user:123")
    .await?;

// Range scan
let results = db.kv()
    .table("users")
    .scan()
    .range(b"user:100"..b"user:200")
    .limit(50)
    .execute()
    .await?;

// Prefix scan
let results = db.kv()
    .table("logs")
    .scan()
    .prefix(b"2024-01-")
    .execute()
    .await?;
```

#### Document Queries

```rust
// Insert document
let doc_id = db.documents()
    .collection("users")
    .insert(json!({
        "name": "Alice",
        "email": "alice@example.com",
        "age": 30
    }))
    .await?;

// Query with filter
let users = db.documents()
    .collection("users")
    .query()
    .filter(Filter::and(vec![
        Filter::eq("age", 30),
        Filter::gt("created_at", "2024-01-01"),
    ]))
    .sort_by("name", SortOrder::Asc)
    .limit(10)
    .execute()
    .await?;

// Complex filter
let results = db.documents()
    .collection("products")
    .query()
    .filter(
        Filter::or(vec![
            Filter::and(vec![
                Filter::eq("category", "electronics"),
                Filter::gt("price", 100.0),
            ]),
            Filter::eq("featured", true),
        ])
    )
    .execute()
    .await?;
```

#### Graph Queries

```rust
// Create nodes and edges
let alice = db.graph()
    .graph("social")
    .create_node()
    .properties(json!({"name": "Alice", "age": 30}))
    .execute()
    .await?;

let bob = db.graph()
    .graph("social")
    .create_node()
    .properties(json!({"name": "Bob", "age": 25}))
    .execute()
    .await?;

db.graph()
    .graph("social")
    .create_edge(alice, bob)
    .edge_type("FOLLOWS")
    .properties(json!({"since": "2024-01-01"}))
    .execute()
    .await?;

// Traverse graph
let friends = db.graph()
    .graph("social")
    .traverse()
    .start(alice)
    .direction(Direction::Outgoing)
    .edge_types(vec!["FOLLOWS", "FRIEND_OF"])
    .max_depth(2)
    .execute()
    .await?;

// Pattern matching
let results = db.graph()
    .graph("social")
    .match_pattern()
    .pattern("(a:Person)-[:FOLLOWS]->(b:Person)-[:FOLLOWS]->(c:Person)")
    .where_clause(Filter::eq("a.name", "Alice"))
    .return_nodes(vec!["a", "b", "c"])
    .execute()
    .await?;
```

#### Vector Queries

```rust
// Insert vector
let vec_id = db.vectors()
    .collection("embeddings")
    .insert()
    .vector(vec![0.1, 0.2, 0.3, /* ... */])
    .metadata(json!({"doc_id": "doc123", "text": "sample text"}))
    .execute()
    .await?;

// Similarity search
let similar = db.vectors()
    .collection("embeddings")
    .search()
    .query_vector(query_embedding)
    .k(10)
    .metric(DistanceMetric::Cosine)
    .execute()
    .await?;

// Hybrid search (vector + filter)
let results = db.vectors()
    .collection("embeddings")
    .search()
    .query_vector(query_embedding)
    .filter(Filter::eq("category", "technology"))
    .k(10)
    .execute()
    .await?;
```

### 2. Query Builder Pattern

```rust
pub struct QueryBuilder<T> {
    query: Query,
    _phantom: PhantomData<T>,
}

impl<T> QueryBuilder<T> {
    pub fn filter(mut self, filter: Filter) -> Self {
        self.query.filter = Some(filter);
        self
    }
    
    pub fn sort_by(mut self, field: &str, order: SortOrder) -> Self {
        self.query.sort.push(SortSpec {
            field: field.to_string(),
            order,
        });
        self
    }
    
    pub fn limit(mut self, limit: usize) -> Self {
        self.query.limit = Some(limit);
        self
    }
    
    pub fn offset(mut self, offset: usize) -> Self {
        self.query.offset = Some(offset);
        self
    }
    
    pub async fn execute(self) -> Result<Vec<T>> {
        // Compile to query plan
        let plan = self.compile()?;
        
        // Execute plan
        let executor = QueryExecutor::new();
        executor.execute(plan).await
    }
}
```

### 3. Filter DSL

```rust
pub enum Filter {
    // Comparison
    Eq(String, Value),
    Ne(String, Value),
    Gt(String, Value),
    Gte(String, Value),
    Lt(String, Value),
    Lte(String, Value),
    
    // Membership
    In(String, Vec<Value>),
    NotIn(String, Vec<Value>),
    
    // Pattern matching
    Like(String, String),
    Regex(String, String),
    
    // Null checks
    IsNull(String),
    IsNotNull(String),
    
    // Logical
    And(Vec<Filter>),
    Or(Vec<Filter>),
    Not(Box<Filter>),
    
    // Array operations
    Contains(String, Value),
    ContainsAll(String, Vec<Value>),
    ContainsAny(String, Vec<Value>),
    
    // Nested document
    Nested(String, Box<Filter>),
}

impl Filter {
    // Convenience constructors
    pub fn eq(field: impl Into<String>, value: impl Into<Value>) -> Self {
        Filter::Eq(field.into(), value.into())
    }
    
    pub fn and(filters: Vec<Filter>) -> Self {
        Filter::And(filters)
    }
    
    pub fn or(filters: Vec<Filter>) -> Self {
        Filter::Or(filters)
    }
}
```

### 4. Query AST (Internal Representation)

```rust
pub struct Query {
    pub source: QuerySource,
    pub filter: Option<Filter>,
    pub projection: Option<Projection>,
    pub sort: Vec<SortSpec>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

pub enum QuerySource {
    Table(TableId),
    Collection(CollectionId),
    Graph { graph_id: GraphId, pattern: GraphPattern },
    VectorSearch { collection: CollectionId, query: VectorQuery },
    Join { left: Box<Query>, right: Box<Query>, condition: JoinCondition },
}

pub struct GraphPattern {
    pub nodes: Vec<NodePattern>,
    pub edges: Vec<EdgePattern>,
    pub constraints: Vec<Filter>,
}

pub struct VectorQuery {
    pub vector: Vec<f32>,
    pub k: usize,
    pub metric: DistanceMetric,
    pub ef_search: Option<usize>,
}
```

### 5. Query Compilation

```rust
pub struct QueryCompiler {
    optimizer: QueryOptimizer,
}

impl QueryCompiler {
    pub fn compile(&self, query: Query) -> Result<QueryPlan> {
        // Parse and validate
        self.validate(&query)?;
        
        // Generate logical plan
        let logical_plan = self.generate_logical_plan(query)?;
        
        // Optimize
        let optimized = self.optimizer.optimize(logical_plan)?;
        
        // Generate physical plan
        let physical_plan = self.generate_physical_plan(optimized)?;
        
        Ok(physical_plan)
    }
    
    fn generate_logical_plan(&self, query: Query) -> Result<LogicalPlan> {
        let mut plan = LogicalPlan::new();
        
        // Add source operator
        plan.add_operator(self.create_source_operator(query.source)?);
        
        // Add filter if present
        if let Some(filter) = query.filter {
            plan.add_operator(Operator::Filter(filter));
        }
        
        // Add projection if present
        if let Some(projection) = query.projection {
            plan.add_operator(Operator::Project(projection));
        }
        
        // Add sort if present
        if !query.sort.is_empty() {
            plan.add_operator(Operator::Sort(query.sort));
        }
        
        // Add limit/offset
        if let Some(limit) = query.limit {
            plan.add_operator(Operator::Limit { limit, offset: query.offset.unwrap_or(0) });
        }
        
        Ok(plan)
    }
}
```

### 6. Optional JSON Query Language (Future)

```json
{
  "collection": "users",
  "filter": {
    "and": [
      {"eq": {"field": "age", "value": 30}},
      {"gt": {"field": "created_at", "value": "2024-01-01"}}
    ]
  },
  "sort": [
    {"field": "name", "order": "asc"}
  ],
  "limit": 10
}
```

```rust
pub fn parse_json_query(json: &str) -> Result<Query> {
    let value: serde_json::Value = serde_json::from_str(json)?;
    
    let collection = value["collection"].as_str()
        .ok_or(Error::InvalidQuery)?;
    
    let filter = if let Some(filter_json) = value.get("filter") {
        Some(parse_filter(filter_json)?)
    } else {
        None
    };
    
    // Parse other fields...
    
    Ok(Query {
        source: QuerySource::Collection(collection.into()),
        filter,
        // ...
    })
}
```

### 7. SQL-like Syntax (Future)

```sql
-- Document queries
SELECT * FROM users
WHERE age = 30 AND created_at > '2024-01-01'
ORDER BY name ASC
LIMIT 10;

-- Graph queries (extended syntax)
MATCH (a:Person)-[:FOLLOWS]->(b:Person)
WHERE a.name = 'Alice'
RETURN b.name, b.age;

-- Vector queries (extended syntax)
SELECT * FROM embeddings
WHERE VECTOR_DISTANCE(embedding, $query_vector, 'cosine') < 0.5
ORDER BY VECTOR_DISTANCE(embedding, $query_vector, 'cosine')
LIMIT 10;
```

### 8. Cross-Model Queries

```rust
// Combine graph traversal with document retrieval
let user_docs = db.graph()
    .graph("social")
    .traverse()
    .start(alice_id)
    .direction(Direction::Outgoing)
    .edge_types(vec!["FOLLOWS"])
    .max_depth(2)
    .execute()
    .await?
    .into_iter()
    .map(|node_id| {
        db.documents()
            .collection("users")
            .get(node_id)
    })
    .collect::<Vec<_>>();

// Vector search with graph context
let similar_connected = db.vectors()
    .collection("embeddings")
    .search()
    .query_vector(query_embedding)
    .k(100)
    .execute()
    .await?
    .into_iter()
    .filter(|result| {
        // Check if connected in graph
        db.graph()
            .graph("social")
            .has_path(user_id, result.metadata["user_id"])
            .max_depth(3)
            .execute()
            .await
            .unwrap_or(false)
    })
    .take(10)
    .collect::<Vec<_>>();
```

## Consequences

### Positive

* **Type safety** - Compile-time error checking
* **Embeddability** - Natural integration with application code
* **Composability** - Build complex queries from simple parts
* **Performance** - No parsing overhead for programmatic queries
* **SDK generation** - Easy to generate bindings for other languages
* **Flexibility** - Can add DSL later without breaking existing code
* **Optimization** - Query AST enables sophisticated optimization

### Negative

* **Less ad-hoc querying** - No REPL-friendly query language initially
* **Verbosity** - Programmatic API can be more verbose than SQL
* **Learning curve** - Developers must learn API instead of familiar SQL
* **Tooling** - Fewer existing tools for programmatic queries

### Risks

* **API stability** - Changes to API are breaking changes
* **Complexity** - Query builder can become complex for advanced queries
* **DSL fragmentation** - Multiple query languages could confuse users

## Alternatives Considered

### 1. SQL as Primary Interface

**Rejected** - SQL doesn't handle graph and vector operations well. Would require many extensions and non-standard syntax.

### 2. GraphQL

**Rejected** - Designed for API queries, not database operations. Doesn't support transactions or complex graph traversals.

### 3. Cypher (Neo4j)

**Rejected** - Graph-specific, doesn't handle documents or vectors well. Would need significant extensions.

### 4. MongoDB Query Language

**Rejected** - Document-focused, limited graph support. Not type-safe.

### 5. Custom DSL Only

**Rejected** - Requires learning new syntax, harder to embed, no compile-time checking.

## Implementation Notes

### Phase 1: Core API (Week 15)
- Implement query builder for documents
- Add filter DSL
- Create query AST

### Phase 2: Graph API (Week 19)
- Add graph query builders
- Implement pattern matching
- Create traversal API

### Phase 3: Vector API (Week 24)
- Add vector search builders
- Implement hybrid queries
- Create similarity API

### Phase 4: Query Optimization (Week 30)
- Implement query compiler
- Add optimization rules
- Create execution engine

### Phase 5: Optional DSL (Future)
- Design JSON query language
- Implement SQL parser
- Add query language documentation

## Related ADRs

* [ADR-0006: Key-Value, Document, and Graph Support](ADR-0006-Key-Value-Document-Graph-Support.md)
* [ADR-0016: Graph Query Semantics](ADR-0016-Graph-Query-Semantics.md)
* [ADR-0017: Hybrid Query Execution](ADR-0017-Hybrid-Query-Execution.md)
* [ADR-0025: Core API Specifications](ADR-0025-Core-API-Specifications.md)

## References

* Rust query builder patterns
* LINQ (Language Integrated Query)
* SQLAlchemy Core
* Diesel query builder
* Cypher query language

---

**Next Steps:**
1. Design core query builder API
2. Implement filter DSL
3. Create query AST
4. Build query compiler
5. Add optimization framework
6. Document query patterns
