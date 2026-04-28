```markdown
# Graph Query Compiler (Rust)
## Gremlin / Cypher / GQL → Common Graph Bytecode (CGB)

---

# 1. Vision

Build a unified Rust-based query compiler that:

- Accepts:
  - Gremlin
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

Multiple frontends → Unified AST → Logical Graph Algebra → Bytecode → Interpreter / Adapter

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
           Interpreter VM / Engine Adapter
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

### Design Goals

- Language-neutral
- Register-based
- Strongly typed
- Logical (not physical)
- Serializable & versioned
- Deterministic execution semantics

---

# 4. Unified AST

All frontends must compile into this shared AST.

### Core AST Nodes

- `Query`
- `MatchClause`
- `Pattern`
- `NodePattern`
- `RelationshipPattern`
- `WhereClause`
- `Projection`
- `Aggregation`
- `OrderBy`
- `Limit`
- `Subquery`
- `TraversalStep` (for Gremlin)

### Rust Sketch

```rust
pub enum AstNode {
    Query(Query),
    Match(MatchClause),
    Pattern(Pattern),
    Expr(Expr),
}
````

---

# 5. Frontend Parsers

---

## Gremlin Frontend

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

## Cypher Frontend

* Use ANTLR grammar or pest
* Implement visitor → AST
* Handle variable scoping and WITH clauses

Output:
Cypher → Unified AST

---

## GQL Frontend

* Start with MATCH / SELECT subset
* Align closely with Cypher frontend

Output:
GQL → Unified AST

---

# 6. Semantic Analyzer

Validates and annotates AST.

### Responsibilities

* Variable resolution
* Scope validation
* Type inference
* Pattern normalization
* Aggregation validation

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

# 7. Logical Graph Algebra IR

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

# 8. Common Graph Bytecode (CGB)

CGB is:

* Register-based
* Strongly typed
* Logical
* Deterministic
* Serializable
* Versioned

---

# 9. Bytecode Operator Definitions

Each instruction:

```rust
pub struct Instruction {
    pub opcode: OpCode,
    pub args: Vec<Operand>,
}
```

Registers identified by `RegId(u16)`.

---

## 9.1 Runtime Value Model

```rust
pub enum Value {
    Node(NodeId),
    Edge(EdgeId),
    Path(Vec<NodeId>, Vec<EdgeId>),
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    List(Vec<Value>),
    Map(HashMap<String, Value>),
    Null,
}
```

---

## 9.2 Graph Operators

### ScanNode

```rust
ScanNode {
    out: RegId,
    label: Option<LabelId>,
}
```

### ScanEdge

```rust
ScanEdge {
    out: RegId,
    label: Option<LabelId>,
}
```

### Expand

```rust
Expand {
    from: RegId,
    direction: Direction,
    edge_label: Option<LabelId>,
    out: RegId,
}
```

### MatchPattern

```rust
MatchPattern {
    pattern_id: PatternId,
    bindings: Vec<RegId>,
}
```

### OptionalExpand

Preserves input row when no match.

### PathConstruct

```rust
PathConstruct {
    nodes: Vec<RegId>,
    edges: Vec<RegId>,
    out: RegId,
}
```

---

## 9.3 Relational Operators

### Filter

```rust
Filter {
    predicate: ExprId,
}
```

### Project

```rust
Project {
    expressions: Vec<(SymbolId, ExprId)>,
}
```

### Aggregate

```rust
Aggregate {
    group_keys: Vec<ExprId>,
    aggregations: Vec<Aggregation>,
}
```

Supported:

* COUNT
* SUM
* AVG
* MIN
* MAX
* COLLECT

### Sort

```rust
Sort {
    keys: Vec<SortKey>,
}
```

### Limit

```rust
Limit {
    count: usize,
}
```

### Distinct

Removes duplicate rows.

### Apply

```rust
Apply {
    subplan: PlanId,
    correlation: Vec<(OuterReg, InnerReg)>,
}
```

---

## 9.4 Expression Operators

Expressions compiled separately and referenced via `ExprId`.

Supported:

* Arithmetic
* Boolean logic
* Comparisons
* Property access
* Function calls
* List construction
* Map construction
* Null-safe operations

---

# 10. Interpreter Architecture

---

## 10.1 Execution Model

Pull-based Volcano iterator model.

```rust
trait Operator {
    fn open(&mut self, ctx: &mut ExecContext);
    fn next(&mut self, ctx: &mut ExecContext) -> Option<Row>;
    fn close(&mut self, ctx: &mut ExecContext);
}
```

---

## 10.2 Execution Context

```rust
pub struct ExecContext<'a> {
    pub graph: &'a dyn GraphStorage,
    pub registers: RegisterFile,
    pub memory: MemoryManager,
}
```

---

## 10.3 Register File

```rust
pub struct RegisterFile {
    values: Vec<Value>,
}
```

Fixed-size per query.

---

## 10.4 GraphStorage Trait

```rust
pub trait GraphStorage {
    fn scan_nodes(&self, label: Option<LabelId>) -> NodeIterator;
    fn scan_edges(&self, label: Option<LabelId>) -> EdgeIterator;

    fn expand(
        &self,
        node: NodeId,
        direction: Direction,
        label: Option<LabelId>,
    ) -> NodeIterator;

    fn get_property(&self, element: ElementId, key: PropertyId) -> Value;
}
```

Backends:

* In-memory graph
* Adapter to external DB
* Distributed storage

---

## 10.5 Plan Builder

```rust
pub struct PlanBuilder;

impl PlanBuilder {
    pub fn build(program: &Program) -> Box<dyn Operator>;
}
```

Responsibilities:

* Convert linear bytecode → operator tree
* Wire inputs/outputs
* Allocate registers
* Inject expression evaluators

---

## 10.6 Streaming vs Blocking

Streaming:

* ScanNode
* Expand
* Filter
* Project

Blocking:

* Aggregate
* Sort
* Distinct

Future:

* Spill to disk
* Parallel execution

---

# 11. Repository Structure

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

# 12. MVP Scope

* ScanNode
* Expand
* Filter
* Project
* Aggregate (COUNT only)
* Limit
* In-memory storage
* Cypher frontend only

Then expand to:

* Gremlin
* GQL
* Subqueries
* Optional match
* Cost-based optimization

---

# 13. Long-Term Vision

* Cost-based optimizer
* Index pushdown
* Distributed execution
* Query federation
* Plan caching
* WASM backend
* Enterprise-grade runtime

---

# Final Objective

Create a Rust-native **Graph Query LLVM**:

* Portable
* Engine-agnostic
* Safe
* Extensible
* High performance
* Production-ready

---

```
```
