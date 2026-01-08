# ADR-0010: Authentication, Authorization, and Access Control

## Status

Proposed

## Context

Nanograph may operate in multi-tenant and distributed environments.

## Decision

Adopt **Attribute-Based Access Control (ABAC)** with optional Policy-Based Access Control (PBAC) overlays.

## Decision Drivers

* Flexibility
* Fine-grained control
* Future-proofing

## Design

* Identity via tokens or certificates
* Policies evaluated at request time
* Table and operation-level enforcement

## Consequences

### Positive

* Powerful security model

### Negative

* Policy complexity

## Alternatives Considered

* RBAC only (rejected)

## Related ADRs

* [ADR-0009: Frontend & Backend Separation, Embedding, and Security](ADR-0009-Frontend-Backend-Separation-Embedding-Security.md)
