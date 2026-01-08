# ADR-0019: Semantic Ranking and Scoring Strategy

## Status

Proposed

## Context

Semantic search results often combine lexical relevance, structural constraints, and vector similarity. A clear ranking model is required to ensure explainable and tunable results.

## Decision

Adopt a **composable, multi-signal ranking model** where scores from different query stages are combined.

## Decision Drivers

* Result quality
* Explainability
* Tunability

## Design

* Scoring signals:

    * Vector similarity
    * Keyword relevance
    * Graph proximity
    * User-defined boosts
* Normalization per signal
* Weighted score aggregation
* Optional re-ranking stage

## Consequences

### Positive

* Flexible ranking behavior
* Supports hybrid search well

### Negative

* Requires tuning and defaults

## Alternatives Considered

* Pure vector similarity (rejected)

## Related ADRs

* [ADR-0017: Hybrid Query Execution](ADR-0017-Hybrid-Query-Execution.md)
* [ADR-0018: Embedding Lifecycle and Model Integration](ADR-0018-Embedding-Lifecycle-and-Model-Integration.md)
