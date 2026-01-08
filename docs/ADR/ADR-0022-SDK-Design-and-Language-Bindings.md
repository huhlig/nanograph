# ADR-0022: SDK Design and Language Bindings

## Status

Proposed

## Context

Nanograph is intended to be embedded directly into applications as well as accessed remotely. Developer experience is critical to adoption.

## Decision

Adopt a **core-native SDK-first strategy**, with additional language bindings layered on top.

## Decision Drivers

* Embeddability
* Performance
* Consistency across languages

## Design

* Rust SDK as the canonical implementation
* Foreign language bindings via:

    * FFI (C ABI)
    * gRPC/HTTP clients
* Unified API semantics across SDKs
* Generated client libraries where possible

## Consequences

### Positive

* High performance
* Clear source of truth

### Negative

* FFI maintenance cost

## Alternatives Considered

* Independent SDK implementations (rejected)

## Related ADRs

* [ADR-0015: Query Interface Strategy](ADR-0015-Query-Interface-Strategy.md)
