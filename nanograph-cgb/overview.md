```markdown
# Graph Query Compiler (Rust)
## Gremlin / Cypher / GQL → Common Graph Bytecode (CGB)

---

# 1. Vision

Build a unified Rust-based query compiler that:

- Accepts:
  - Gremlin (Apache TinkerPop)
  - Cypher
  - GQL (ISO standard)
- Produces:
  - **Common Graph Bytecode (CGB)** — a language-neutral intermediate representation
- Targets:
  - Embedded graph engines
  - Remote graph databases
  - Custom execution runtimes
  - Distributed systems

The architecture mirrors LLVM-style compiler design:

Multiple frontends → Unified AST → Logical Graph Algebra → Bytecode → Engine

---

# 2. Core Architecture

```

```
        Gremlin      Cypher      GQL
           |           |          |
        Parser      Parser     Parser
           \           |          /
            \          |         /
               → Unified AST →
                  Semantic Layer
                       ↓
               Logical Graph Algebra
                       ↓
            Common Graph Bytecode (CGB)
                       ↓
           VM / Engine Adapter Layer
```

````

---

# 3. Phased Development Plan

---

## Phase 0 — Research & Formalization (4–6 Weeks)

### Goals

- Map semantic overlap between:
  - Gremlin (imperative traversal)
  - Cypher (declarative pattern matching)
  - GQL (ISO declarative standard)
- Define:
  - Type system
  - Pattern model
  - Traversal algebra
  - Variable scoping rules

### Deliverables

- Semantic compatibility matrix
- Operator equivalence mapping
- CGB draft specification (v0.1)
- MVP feature scope document

---

## Phase 1 — Define Common Graph Bytecode (CGB)

This is the system’s core.

### Design Goals

- Language-neutral
- Register-based (not stack-based)
- Strongly typed
- Logical (not physical)
- Serializable & versioned
- Deterministic execution semantics

---

### 1.1 Instruction Categories

#### Graph Operators
- `ScanNode`
- `ScanEdge`
- `Expand`
- `Match`
- `OptionalMatch`
- `PathConstruct`

#### Relational Operators
- `Filter`
- `Project`
- `Aggregate`
- `Sort`
- `Limit`
- `Distinct`

#### Control Operators
- `Apply` (subqueries)
- `Union`
- `Exists`

#### Expression Operators
- Arithmetic
- Boolean
- Comparison
- Property access
- Function calls

---

### 1.2 Example CGB Program

```json
[
  ["ScanNode", 0, {"label": "Person"}],
  ["Expand", 0, "OUT", "KNOWS", 1],
  ["Filter", "reg1.age > 30"],
  ["Project", {"name": "reg1.name"}]
]
````

Registers:

* `reg0` → node `n`
* `reg1` → node `m`

---

### 1.3 Rust Representation (Conceptual)

```rust
pub enum Instruction {
    ScanNode { out: RegId, label: Option<String> },
    Expand { from: RegId, dir: Direction, label: Option<String>, out: RegId },
    Filter { predicate: Expr },
    Project { mappings: Vec<(String, Expr)> },
    Aggregate { group_keys: Vec<Expr>, aggs: Vec<Aggregation> },
    Limit { count: usize },
}
```

---

## Phase 2 — Unified AST

All frontends must compile into this shared AST.

### Core AST Nodes

* `Query`
* `MatchClause`
* `Pattern`
* `NodePattern`
* `RelationshipPattern`
* `WhereClause`
* `Projection`
* `Aggregation`
* `OrderBy`
* `Limit`
* `Subquery`
* `TraversalStep` (for Gremlin)

---

### Rust Sketch

```rust
pub enum AstNode {
    Query(Query),
    Match(MatchClause),
    Pattern(Pattern),
    Expr(Expr),
}
```

---

## Phase 3 — Frontend Parsers

---

### 3.1 Gremlin Frontend

Strategy:

* Parse traversal DSL subset
* Disallow lambdas (MVP)
* Map traversal steps → AST nodes

Challenges:

* Imperative style
* Nested traversals
* Anonymous traversals

Output:
Gremlin → Unified AST

---

### 3.2 Cypher Frontend

Strategy:

* Use ANTLR grammar or pest
* Implement visitor → AST

Concerns:

* Variable scoping
* WITH clauses
* Pattern semantics

Output:
Cypher → Unified AST

---

### 3.3 GQL Frontend

Strategy:

* Start with MATCH / SELECT subset
* Align closely with Cypher frontend

Output:
GQL → Unified AST

---

## Phase 4 — Semantic Analyzer

This validates and annotates the AST.

### Responsibilities

* Variable resolution
* Scope validation
* Type inference
* Pattern normalization
* Aggregation validation

---

### Symbol Tables

* Query scope
* Subquery scope
* Pattern scope

---

### Type System

```rust
pub enum Type {
    Node,
    Relationship,
    Path,
    Integer,
    Float,
    Boolean,
    String,
    List(Box<Type>),
    Map,
    Null,
}
```

---

## Phase 5 — Logical Graph Algebra IR

Language-neutral algebra layer.

### Operators

* `NodeScan(label)`
* `EdgeScan(label)`
* `Expand(direction, label)`
* `Match(pattern)`
* `Filter(predicate)`
* `Project(expressions)`
* `Aggregate(group_keys, functions)`
* `Sort`
* `Limit`
* `Apply`
* `OptionalMatch`

---

### Rust Example

```rust
pub enum LogicalPlan {
    NodeScan { label: Option<String> },
    Expand { input: Box<LogicalPlan>, dir: Direction, label: Option<String> },
    Filter { input: Box<LogicalPlan>, predicate: Expr },
    Project { input: Box<LogicalPlan>, projections: Vec<(String, Expr)> },
}
```

---

## Phase 6 — Lowering to CGB

LogicalPlan → Linear CGB

Steps:

1. Plan traversal
2. Register allocation
3. Expression lowering
4. Dependency ordering
5. Instruction emission

Deliverable:
Executable CGB program

---

## Phase 7 — Execution Layer

Two possible directions:

### Option A — CGB Interpreter (VM)

* Rust VM
* Pluggable storage backend
* In-memory graph engine

### Option B — Engine Adapters

Translate CGB to:

* Gremlin bytecode
* Cypher execution plan
* Engine-native APIs

---

# 4. Cross-Language Semantic Strategy

| Area                      | Strategy                         |
| ------------------------- | -------------------------------- |
| Imperative vs Declarative | Normalize to declarative algebra |
| Path binding              | Explicit IR operator             |
| Optional match            | Dedicated algebra node           |
| Aggregation               | Graph relational algebra         |
| Subqueries                | Apply operator                   |

---

# 5. Testing Strategy

### 5.1 Golden Query Suite

For each query:

* Gremlin
* Cypher
* GQL

Ensure:

* Same AST
* Same LogicalPlan
* Same Bytecode

---

### 5.2 Property-Based Testing

* Random graph generation
* Random pattern queries
* Compare results across backends

---

### 5.3 Differential Testing

Compare against:

* Neo4j
* Apache TinkerPop reference engine

---

# 6. Suggested Rust Tech Stack

| Layer           | Tool               |
| --------------- | ------------------ |
| Parsing         | pest or ANTLR      |
| Serialization   | serde / prost      |
| IR optimization | Custom rule engine |
| Testing         | proptest           |
| CLI             | clap               |

---

# 7. Suggested Repository Structure

```
graph-compiler/
│
├── crates/
│   ├── ast/
│   ├── parser-gremlin/
│   ├── parser-cypher/
│   ├── parser-gql/
│   ├── semantic/
│   ├── logical-plan/
│   ├── cgb/
│   ├── optimizer/
│   ├── vm/
│   └── adapters/
│
├── tests/
│   ├── golden/
│   ├── property/
│   └── differential/
│
└── cli/
```

---

# 8. Timeline (MVP)

| Phase            | Duration |
| ---------------- | -------- |
| Research         | 1 month  |
| AST + CGB        | 1 month  |
| Cypher frontend  | 1 month  |
| Gremlin frontend | 1 month  |
| VM prototype     | 1 month  |

Total: ~4–5 months for functional MVP

---

# 9. Architectural Principles

1. Frontends are isolated.
2. AST is language-neutral.
3. Logical algebra is canonical.
4. Bytecode is stable & versioned.
5. Optimizer is frontend-agnostic.
6. Execution is pluggable.

---

# 10. Long-Term Goals

* Cost-based optimizer
* Index pushdown
* Distributed execution
* Query federation
* Plan caching
* WASM execution target
* Enterprise-grade runtime

---

# Final Objective

Create a **Graph Query LLVM** in Rust:

* Portable
* Engine-agnostic
* Safe
* Extensible
* High performance
* Production-ready

---

```
```
