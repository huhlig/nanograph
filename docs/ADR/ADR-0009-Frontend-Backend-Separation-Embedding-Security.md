# ADR-0009: Frontend & Backend Separation, Embedding, and Security

## Status

Proposed

## Context

Nanograph must run embedded or remotely while maintaining secure communication.

## Decision

Enforce a **strict frontend/backend separation** with well-defined APIs. Use **mTLS** for inter-process and network communication.

## Decision Drivers

* Security
* Deployment flexibility
* Clear boundaries

## Design

* Backend exposes gRPC/HTTP APIs
* Embedded mode bypasses transport
* mTLS for node and client auth

## Consequences

### Positive

* Strong security model
* Clean architecture

### Negative

* Added setup complexity

## Alternatives Considered

* Plain TLS or shared secrets (rejected)

## Related ADRs

* [ADR-0010: Authentication, Authorization, and Access Control](ADR-0010-Authentication-Authorization-Access-Control.md)
