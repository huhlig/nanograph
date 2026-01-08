# ADR-0017: Hybrid Query Execution (KV, Document, Graph, Vector)

## Status

Proposed

## Context

Nanograph queries frequently combine multiple paradigms, such as filtering documents, traversing graphs, and ranking results via vector similarity.

## Decision

Introduce a **hybrid query execution pipeline** that composes multiple query operators into an explicit execution plan.

## Decision Drivers

* Multi-model query support
* Performance transparency
* Optimizer extensibility

## Design

* Queries compile into an operator DAG
* Operator types:

    * KV scan / lookup
    * Document filter
    * Graph traversal
    * Vector similarity search
* Execution is pull-based with backpressure
* Cost-based optimization deferred; rule-based ordering initially

## Consequences

### Positive

* Powerful composability
* Clear execution boundaries

### Negative

* More complex query engine

## Alternatives Considered

* Monolithic query engine per model (rejected)

## Related ADRs

* [ADR-0015: Query Interface Strategy](ADR-0015-Query-Interface-Strategy.md)
* [ADR-0016: Graph Query Semantics](ADR-0016-Graph-Query-Semantics.md)
