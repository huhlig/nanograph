# Nanograph Glossary

This glossary defines key terms, concepts, and acronyms used throughout the Nanograph documentation.

---

## A

### ACID
**Atomicity, Consistency, Isolation, Durability** - The four key properties that guarantee database transactions are processed reliably.

### ADR
**Architectural Decision Record** - A document that captures an important architectural decision made along with its context and consequences.

### Adjacency List
A graph representation where each node stores a list of its adjacent nodes (neighbors). Nanograph uses adjacency lists to store graph edges efficiently.

### ANN
**Approximate Nearest Neighbor** - Algorithms for finding vectors that are approximately closest to a query vector, trading perfect accuracy for speed.

### API
**Application Programming Interface** - A set of functions and procedures allowing applications to interact with Nanograph.

### ART
**Adaptive Radix Tree** - A space-efficient trie data structure used for indexing ordered data.

### AST
**Abstract Syntax Tree** - A tree representation of the abstract syntactic structure of queries.

---

## B

### B+Tree
A self-balancing tree data structure that maintains sorted data and allows searches, sequential access, insertions, and deletions in logarithmic time.

### Batch Operation
An operation that processes multiple items together for efficiency, reducing overhead compared to individual operations.

### Bloom Filter
A space-efficient probabilistic data structure used to test whether an element is a member of a set, with possible false positives but no false negatives.

### BPT
**B+ Tree** - See B+Tree.

---

## C

### Cache
A high-speed data storage layer that stores a subset of data for faster access.

### Checkpoint
A point in time where the database state is saved to stable storage, enabling recovery.

### Cluster
A group of Nanograph nodes working together to provide distributed database functionality.

### Compaction
The process of merging and reorganizing data files to reclaim space and improve read performance.

### Consensus
Agreement among distributed nodes on a single data value or state, typically achieved through algorithms like Raft.

### Consistency
The guarantee that all nodes in a distributed system see the same data at the same time.

### CRC32C
**Cyclic Redundancy Check** - A checksum algorithm used to detect data corruption.

### CRUD
**Create, Read, Update, Delete** - The four basic operations for persistent storage.

---

## D

### Deadlock
A situation where two or more transactions are waiting for each other to release locks, preventing any from proceeding.

### Document
A self-contained data structure, typically in JSON format, containing fields and values.

### Document Store
A database that stores, retrieves, and manages document-oriented information.

### DSL
**Domain-Specific Language** - A specialized language designed for a specific application domain.

### Durability
The guarantee that once a transaction is committed, it will remain committed even in the event of system failure.

---

## E

### Edge
A connection between two nodes in a graph, optionally with properties and direction.

### Embedding
A vector representation of data (text, images, etc.) in a high-dimensional space, used for semantic similarity.

### Embedded Mode
Running Nanograph as a library within an application process, without a separate server.

### Eventual Consistency
A consistency model where updates to a distributed system will eventually propagate to all nodes.

---

## F

### Failover
The automatic switching to a redundant system when the primary system fails.

### Follower
A replica node in a Raft group that replicates data from the leader but doesn't accept writes directly.

### fsync
A system call that forces all buffered modifications to a file to be written to disk.

---

## G

### Garbage Collection (GC)
The process of reclaiming storage occupied by data that is no longer needed.

### Graph
A data structure consisting of nodes (vertices) and edges (relationships) between them.

### Graph Database
A database optimized for storing and querying graph-structured data.

### gRPC
**gRPC Remote Procedure Call** - A high-performance RPC framework using HTTP/2 and Protocol Buffers.

---

## H

### Hash Function
A function that maps data of arbitrary size to fixed-size values, used for partitioning and indexing.

### HNSW
**Hierarchical Navigable Small World** - An algorithm for approximate nearest neighbor search in high-dimensional spaces.

### Hot Path
The most frequently executed code path in a system, critical for performance optimization.

### Hybrid Query
A query that combines multiple query types, such as vector similarity search with metadata filtering.

---

## I

### Idempotent
An operation that produces the same result regardless of how many times it is executed.

### Index
A data structure that improves the speed of data retrieval operations.

### Isolation
The property ensuring that concurrent transactions don't interfere with each other.

### IVF
**Inverted File Index** - A vector indexing method that partitions the vector space into clusters.

---

## J

### JSON
**JavaScript Object Notation** - A lightweight data interchange format.

---

## K

### Key-Value Store
A database that stores data as a collection of key-value pairs.

### KV
Abbreviation for Key-Value.

---

## L

### Latency
The time delay between a request and its response.

### Leader
The node in a Raft group that accepts writes and coordinates replication to followers.

### Lease
A time-limited grant of exclusive access to a resource.

### Linearizability
The strongest consistency model, ensuring operations appear to occur instantaneously at some point between invocation and response.

### LSM Tree
**Log-Structured Merge Tree** - A data structure optimized for write-heavy workloads.

---

## M

### Memtable
An in-memory data structure where writes are initially stored before being flushed to disk.

### Metadata
Data that provides information about other data.

### Metric
A measurement of system behavior or performance.

### mTLS
**Mutual TLS** - A security protocol where both client and server authenticate each other.

### Multi-Model Database
A database that supports multiple data models (KV, document, graph, vector) in a single system.

### MVCC
**Multi-Version Concurrency Control** - A concurrency control method that maintains multiple versions of data to allow concurrent access.

---

## N

### Node
1. In graphs: A vertex or entity in a graph structure.
2. In clusters: A server instance in a distributed system.

---

## O

### OLAP
**Online Analytical Processing** - Workloads focused on complex queries and analytics.

### OLTP
**Online Transaction Processing** - Workloads focused on transaction-oriented applications.

### ONNX
**Open Neural Network Exchange** - An open format for representing machine learning models.

---

## P

### Partition
A division of data across multiple nodes or storage units.

### Paxos
A consensus algorithm for distributed systems, more complex than Raft.

### Persistence
The characteristic of data that outlives the process that created it.

### Primary Key
A unique identifier for a record in a database.

### Protocol Buffers
A language-neutral, platform-neutral mechanism for serializing structured data.

---

## Q

### Query
A request for data or information from a database.

### Query Optimizer
A component that determines the most efficient way to execute a query.

### Query Plan
A sequence of operations to execute a query.

### Quorum
The minimum number of nodes that must agree for an operation to succeed in a distributed system.

---

## R

### Raft
A consensus algorithm designed to be more understandable than Paxos while providing equivalent guarantees.

### Range Query
A query that retrieves all records within a specified key range.

### RBAC
**Role-Based Access Control** - An access control method based on user roles.

### Read Committed
An isolation level where transactions only see data committed before they started.

### Rebalancing
The process of redistributing data across nodes to maintain balance.

### Replica
A copy of data maintained on multiple nodes for fault tolerance.

### Replication
The process of copying data across multiple nodes.

### Replication Factor
The number of copies of data maintained in a distributed system.

---

## S

### SDK
**Software Development Kit** - A collection of tools for developing applications.

### Semantic Search
Search based on meaning and context rather than exact keyword matching.

### Serializable
The strongest isolation level, equivalent to serial execution of transactions.

### Shard
A horizontal partition of data in a distributed database.

### Sharding
The process of distributing data across multiple shards.

### Snapshot
A consistent point-in-time copy of data.

### Snapshot Isolation (SI)
An isolation level where transactions see a consistent snapshot of data as of their start time.

### Split Brain
A failure scenario where a distributed system splits into multiple independent groups.

### SSTable
**Sorted String Table** - An immutable, sorted file format used in LSM trees.

### Standalone Mode
Running Nanograph as a separate server process.

---

## T

### Table
A logical namespace for organizing related data.

### Throughput
The amount of work performed per unit of time.

### Timestamp
A value representing a specific point in time.

### TLS
**Transport Layer Security** - A cryptographic protocol for secure communication.

### Transaction
A unit of work performed against a database, following ACID properties.

### Traversal
The process of visiting nodes in a graph by following edges.

### TTL
**Time To Live** - The duration for which data is considered valid.

---

## U

### UUID
**Universally Unique Identifier** - A 128-bit identifier that is unique across space and time.

---

## V

### Vector
An array of numbers representing data in a high-dimensional space.

### Vector Database
A database optimized for storing and querying vector embeddings.

### Vector Index
A data structure for efficient similarity search in vector spaces.

### Versioning
The practice of maintaining multiple versions of data or software.

### VFS
**Virtual File System** - An abstraction layer over file system operations.

---

## W

### WAL
**Write-Ahead Log** - A log of changes written before they are applied to the database, enabling recovery.

### Write Amplification
The ratio of data written to storage versus data written by the application.

### Write Skew
An anomaly where two transactions read overlapping data and make disjoint updates.

---

## Y

### YCSB
**Yahoo! Cloud Serving Benchmark** - A framework for benchmarking database systems.

---

## Acronyms Quick Reference

| Acronym | Full Term |
|---------|-----------|
| ACID | Atomicity, Consistency, Isolation, Durability |
| ADR | Architectural Decision Record |
| ANN | Approximate Nearest Neighbor |
| API | Application Programming Interface |
| ART | Adaptive Radix Tree |
| AST | Abstract Syntax Tree |
| BPT | B+ Tree |
| CRC | Cyclic Redundancy Check |
| CRUD | Create, Read, Update, Delete |
| DSL | Domain-Specific Language |
| GC | Garbage Collection |
| gRPC | gRPC Remote Procedure Call |
| HNSW | Hierarchical Navigable Small World |
| IVF | Inverted File Index |
| JSON | JavaScript Object Notation |
| KV | Key-Value |
| LSM | Log-Structured Merge |
| mTLS | Mutual TLS |
| MVCC | Multi-Version Concurrency Control |
| OLAP | Online Analytical Processing |
| OLTP | Online Transaction Processing |
| ONNX | Open Neural Network Exchange |
| RBAC | Role-Based Access Control |
| SDK | Software Development Kit |
| SI | Snapshot Isolation |
| SST | Sorted String Table |
| TLS | Transport Layer Security |
| TTL | Time To Live |
| UUID | Universally Unique Identifier |
| VFS | Virtual File System |
| WAL | Write-Ahead Log |
| YCSB | Yahoo! Cloud Serving Benchmark |

---

## See Also

* [PROJECT_REQUIREMENTS.md](PROJECT_REQUIREMENTS.md) - Product requirements and vision
* [ARCHITECTURE_APPENDICES.md](ARCHITECTURE_APPENDICES.md) - System invariants and constraints
* [ADR Index](ADR/ADR-0000-Index-of-ADRs.md) - All architectural decisions
* [Implementation Plan](DEV/IMPLEMENTATION_PLAN.md) - Development roadmap

---

*This glossary is a living document. Please submit updates as new terms are introduced or definitions need clarification.*