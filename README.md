# Nanograph – Embeddable AI Database

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Actions Status](https://github.com/huhlig/nanograph/workflows/rust/badge.svg)](https://github.com/huhlig/nanograph/actions)

> An embeddable graph and document database for AI and MCP Services.

## Overview

Minerva is a powerful knowledge graph server that provides intelligent document ingestion, database schema extraction, and sophisticated querying capabilities through both MCP and REST APIs. It combines graph database technology with AI-powered extraction to build rich, interconnected knowledge representations from diverse sources including documents, databases, and conceptual models.

### Key Features

- TBD 

## Architecture

- TBD

## Project Structure

```
nanograph/
├── src/
│   ├── lib.rs               # Library File
│   ├── storage/             # Storage Abstraction
│   │   ├── memory.rs        # Memory Storage
│   │   ├── localfile.rs     # Local File Storage
│   │   └── distributed.rs   # Distributed Cluster Storage
│   ├── graph/               # Graph Model
│   │   ├── graph.rs         # Knowledge graph service
│   │   ├── ingestion.rs     # Document ingestion
│   │   ├── query.rs         # Query engine
│   │   ├── embedding.rs     # Embedding generation
│   │   ├── intent.rs        # Intent extraction
│   │   └── schema.rs        # Schema management
│   ├── extractors/          # Document extractors
│   │   ├── rule_based/      # Rule-based extractors
│   │   └── llm_based/       # LLM-based extractors
│   ├── tools/               # MCP tools
│   └── models/              # Data models
│       ├── node.rs          # Node types (48 types)
│       ├── edge.rs          # Edge types (22 types)
│       └── types.rs         # Type definitions
├── schema-extractor/        # Java schema extraction tool
│   ├── src/main/java/       # Java source code
│   ├── pom.xml              # Maven configuration
│   └── README.md            # Usage guide
├── docs/                    # Documentation
├── config.toml              # Configuration file
└── Cargo.toml              # Dependencies
```

## Configuration

- TBD

## License

This project is licensed under [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be licensed as above, without any additional terms or conditions.

## Acknowledgments

- TBD

## Support

- **Documentation**: See the [docs](docs/) directory
- **Issues**: Report bugs on [GitHub Issues](https://github.com/huhlig/minerva/issues)
- **Discussions**: Join our [GitHub Discussions](https://github.com/huhlig/minerva/discussions)

---

**Status**: 🚧 Under Active Development

This project is currently in active development. APIs and features may change.