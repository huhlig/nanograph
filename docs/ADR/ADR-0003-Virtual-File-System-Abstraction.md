---
parent: ADR
nav_order: 0003
title: Virtual File System Abstraction
status: accepted
date: 2026-01-05
deciders: Hans W. Uhlig
---

# ADR-0003: Virtual File System Abstraction

## Status

Accepted

## Context

Nanograph must operate across multiple deployment environments (embedded, standalone, containerized, cloud, edge). Direct coupling to OS-specific file APIs would limit portability, testing, and extensibility. Storage components (WAL, indexes, tables) require a uniform interface for persistence.

## Decision

Introduce a **Virtual File System (VFS) abstraction** that encapsulates all filesystem interactions. All storage engines interact exclusively through the VFS layer.

## Decision Drivers

* Portability across OSes and environments
* Testability (in-memory and fault-injection FS)
* Support for future backends (object storage, remote FS)
* Security and sandboxing

## Design

* Core traits/interfaces: `FileSystem`, `File`, `VirtualFilesystem`, `VirtualFile`
* Operations: open, read, write, fsync, mmap (optional), lock
* Reference implementations:

    * Local OS filesystem
    * In-memory filesystem

## Consequences

### Positive

* Clean separation of storage logic and environment
* Enables deterministic testing
* Allows future remote or encrypted FS backends

### Negative

* Slight abstraction overhead
* Must carefully expose durability semantics

## Alternatives Considered

* Direct OS filesystem access (rejected)
* External embedded DB FS abstractions (rejected for control reasons)

## Related ADRs

* [ADR-0004: Storage File Formats](ADR-0004-Storage-File-Formats.md)
* [ADR-0005: Write Ahead Log Support](ADR-0005-Write-Ahead-Log-Support.md)
