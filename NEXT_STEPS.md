# Nanograph - Next Steps & Priorities

**Last Updated:** 2026-01-08  
**Status:** Active Development - Phase 1  
**See also:** [PROJECT_STATUS.md](PROJECT_STATUS.md) for comprehensive status

---

## 🎯 Current Focus: Complete Core Storage Engines

### Critical Priority (This Week)

#### 1. Complete LSM Compaction (Est: 1 week) 🔴
**Status:** Partial implementation, needs completion  
**Blocking:** Production-ready LSM storage engine

**Tasks:**
- [ ] Implement full SSTable iteration for compaction
- [ ] Add background compaction thread
- [ ] Complete leveled compaction strategy
- [ ] Implement atomic SSTable replacement
- [ ] Add compaction throttling
- [ ] Optimize compaction scheduling
- [ ] Add compaction statistics and monitoring

**Success Criteria:**
- Background compaction running
- Write amplification < 20x
- Compaction overhead < 10% CPU

---

## High Priority (Next 2 Weeks)

### 2. LSM Bloom Filters (Est: 2-3 days) 🟠
**Tasks:**
- [ ] Implement blocked bloom filter (10 bits/key)
- [ ] Integrate with SSTable reads
- [ ] Add bloom filter effectiveness metrics
- [ ] Optimize false positive rate

**Success Criteria:**
- Bloom filter FPR < 1%
- Read performance improved for non-existent keys

### 3. Metrics Integration (Est: 2-3 days) 🟠
**Tasks:**
- [ ] Integrate metrics crate across all modules
- [ ] Instrument VFS operations
- [ ] Instrument WAL operations
- [ ] Instrument LSM operations
- [ ] Add histogram tracking for latencies
- [ ] Create metrics export interface

**Success Criteria:**
- All operations instrumented
- Metrics exportable to Prometheus format

### 4. VFS Completion (Est: 3-4 days) 🟡
**Tasks:**
- [ ] Implement fsync semantics for LocalFilesystem
- [ ] Improve file locking mechanisms
- [ ] Complete VFS test suite (100+ test cases)
- [ ] Add performance benchmarks for file operations

**Success Criteria:**
- Durability guarantees verified
- All VFS tests passing

### 5. WAL Enhancements (Est: 3-4 days) 🟡
**Tasks:**
- [ ] Implement snapshot coordination
- [ ] Add WAL compaction/truncation
- [ ] Complete crash recovery tests
- [ ] Add performance benchmarks (write throughput, recovery time)

**Success Criteria:**
- Crash recovery tests passing
- WAL overhead < 10% of write throughput

---

## Medium Priority (Next Month)

### 6. Testing Infrastructure (Est: 3-4 days)
**Tasks:**
- [ ] Set up property-based testing framework (proptest)
- [ ] Create deterministic simulation harness foundation
- [ ] Establish benchmark suite structure
- [ ] Build fault injection framework skeleton

**Success Criteria:**
- Testing framework operational
- Benchmark suite running in CI

### 7. KV API Completion (Est: 2-3 days)
**Tasks:**
- [ ] Define transaction primitives interface (single-shard)
- [ ] Create comprehensive trait test suite

**Success Criteria:**
- All trait methods tested
- Transaction interface complete

---

## Technical Debt

### Critical Technical Debt
1. **LSM MemTable:** Replace BTreeMap with lock-free skip list
2. **LSM Compression:** Add actual compression implementation
3. **LSM Checksums:** Implement CRC32C validation
4. **File Management:** Add proper file cleanup and rotation
5. **Error Recovery:** Add automatic recovery strategies

### Project Structure Debt
1. **Crate Organization:** Finalize per Appendix B in IMPLEMENTATION_PLAN.md
2. **Workspace:** Create proper workspace Cargo.toml structure
3. **Dependencies:** Define inter-crate dependency rules
4. **Documentation:** Set up automated documentation generation

---

## Future Phases (3+ Months Out)

### Phase 2: Distributed Consensus (6 weeks)
- Raft integration (tikv/raft-rs)
- Sharding & placement
- Distributed operations
- Multi-node testing

### Phase 3: Multi-Model Support (8 weeks)
- Document model
- Graph model
- Indexing infrastructure
- Secondary indexes

### Phase 4: Vector & AI Capabilities (8 weeks)
- Vector storage & indexing (HNSW)
- Embedding pipeline
- Semantic search
- Hybrid queries

### Phase 5+: Production Readiness (12+ weeks)
- APIs & SDKs
- Observability
- Backup & restore
- Operations tooling

---

## Success Metrics

### Phase 1 Targets (Current)
- ✅ LSM MVCC: All 9 tests passing
- ✅ B+Tree MVCC Core: 49/49 tests passing
- ✅ B+Tree Complete: All integration tests passing
- [ ] LSM Compaction: Background compaction operational
- [ ] Write throughput: >100K ops/sec
- [ ] Read latency p99: <1ms
- [ ] Space amplification: <1.5x
- [ ] Write amplification: <20x

### Overall Project Health
- Test Coverage: Target 90%+
- Documentation: All public APIs documented
- Performance: All benchmarks passing
- Stability: No critical bugs

---

## Weekly Priorities

### Week of 2026-01-08
1. **Monday-Wednesday:** Complete LSM compaction (#1)
2. **Thursday-Friday:** Implement bloom filters (#2)

### Week of 2026-01-15
1. Complete bloom filters (#2)
2. Begin metrics integration (#3)
3. Start VFS enhancements (#4)

### Week of 2026-01-22
1. Complete VFS enhancements (#4)
2. Complete WAL enhancements (#5)
3. Set up testing infrastructure (#6)

### Week of 2026-01-29
1. Complete KV API (#7)
2. Address critical technical debt
3. Performance benchmarking

---

## References

- **[PROJECT_STATUS.md](PROJECT_STATUS.md)** - Comprehensive project status
- [Implementation Plan](docs/DEV/IMPLEMENTATION_PLAN.md) - Full project roadmap
- [LSM Next Steps](nanograph-lsm/NEXT_STEPS.md) - LSM-specific details
- [B+Tree Status](nanograph-btree/COMPLETION_STATUS.md) - B+Tree completion details
- [ADR Index](docs/ADR/ADR-0000-Index-of-ADRs.md) - Architecture decisions

---

## Recent Achievements (Last 7 Days)

- ✅ B+Tree implementation complete (49/49 tests passing)
- ✅ LSM MVCC working (9/9 tests passing)
- ✅ Iterator fixes with cursor support
- ✅ Transaction deadlock resolved
- ✅ Documentation consolidated

---

**Note:** This document focuses on immediate priorities and tasks. For comprehensive status including completed work, test results, and detailed component status, see [PROJECT_STATUS.md](PROJECT_STATUS.md).