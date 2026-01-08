# Contributing to Nanograph

Thank you for your interest in contributing to Nanograph! This guide will help you get started with development, understand our workflow, and make meaningful contributions.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Environment](#development-environment)
- [Project Structure](#project-structure)
- [Development Workflow](#development-workflow)
- [Coding Standards](#coding-standards)
- [Testing Guidelines](#testing-guidelines)
- [Documentation](#documentation)
- [Pull Request Process](#pull-request-process)
- [Release Process](#release-process)
- [Getting Help](#getting-help)

---

## Code of Conduct

We are committed to providing a welcoming and inclusive environment. All contributors are expected to:

- Be respectful and considerate
- Welcome newcomers and help them get started
- Focus on constructive feedback
- Assume good intentions
- Respect differing viewpoints and experiences

---

## Getting Started

### Prerequisites

- **Rust:** 1.70 or later (stable toolchain)
- **Git:** For version control
- **IDE:** VS Code with rust-analyzer recommended
- **OS:** Linux, macOS, or Windows with WSL2

### Quick Start

1. **Fork and Clone**
   ```bash
   git clone https://github.com/yourusername/nanograph.git
   cd nanograph
   ```

2. **Build the Project**
   ```bash
   cargo build
   ```

3. **Run Tests**
   ```bash
   cargo test
   ```

4. **Run Benchmarks**
   ```bash
   cargo bench
   ```

---

## Development Environment

### Required Tools

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install additional components
rustup component add rustfmt clippy

# Install cargo tools
cargo install cargo-watch cargo-audit cargo-outdated
```

### Recommended VS Code Extensions

- **rust-analyzer:** Rust language support
- **CodeLLDB:** Debugging support
- **Even Better TOML:** TOML file support
- **Error Lens:** Inline error display
- **GitLens:** Git integration

### Environment Configuration

Create a `.env` file in the project root (optional):

```bash
RUST_LOG=debug
RUST_BACKTRACE=1
```

---

## Project Structure

```
nanograph/
├── backend/              # Backend storage engine
│   ├── src/
│   │   ├── vfs.rs       # Virtual File System
│   │   ├── wal.rs       # Write-Ahead Log
│   │   ├── lsm.rs       # LSM Tree implementation
│   │   ├── art.rs       # Adaptive Radix Tree
│   │   └── bpt.rs       # B+ Tree
│   └── Cargo.toml
├── frontend/             # Frontend API and client
│   ├── src/
│   └── Cargo.toml
├── docs/                 # Documentation
│   ├── ADR/             # Architecture Decision Records
│   ├── DEV/             # Development guides
│   ├── PROJECT_REQUIREMENTS.md
│   ├── ARCHITECTURE_APPENDICES.md
│   └── GLOSSARY.md
├── Cargo.toml           # Workspace configuration
├── CONTRIBUTING.md      # This file
└── README.md
```

### Module Organization

See [Appendix B in IMPLEMENTATION_PLAN.md](docs/DEV/IMPLEMENTATION_PLAN.md#appendix-b-module-dependency-graph) for the complete module dependency graph.

**Key Principles:**
- Each crate has a single, well-defined responsibility
- Dependencies flow downward (no circular dependencies)
- Core storage layer is independent of higher-level abstractions
- Public APIs are clearly separated from internal implementation

---

## Development Workflow

### Branch Strategy

- **main:** Stable, production-ready code
- **develop:** Integration branch for features
- **feature/\*:** Feature development branches
- **fix/\*:** Bug fix branches
- **docs/\*:** Documentation updates

### Creating a Feature Branch

```bash
git checkout develop
git pull origin develop
git checkout -b feature/your-feature-name
```

### Commit Message Format

Follow the [Conventional Commits](https://www.conventionalcommits.org/) specification:

```
<type>(<scope>): <subject>

<body>

<footer>
```

**Types:**
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting, etc.)
- `refactor`: Code refactoring
- `perf`: Performance improvements
- `test`: Adding or updating tests
- `chore`: Maintenance tasks

**Examples:**
```
feat(storage): implement LSM tree compaction

Add leveled compaction strategy for LSM tree with configurable
size ratios and level multipliers.

Closes #123
```

```
fix(wal): correct checksum validation logic

The previous implementation didn't handle partial writes correctly.
This fix ensures checksums are validated for complete entries only.

Fixes #456
```

### Daily Development Cycle

1. **Pull latest changes**
   ```bash
   git checkout develop
   git pull origin develop
   ```

2. **Create/update feature branch**
   ```bash
   git checkout -b feature/my-feature
   ```

3. **Make changes and test**
   ```bash
   # Make your changes
   cargo fmt
   cargo clippy
   cargo test
   ```

4. **Commit changes**
   ```bash
   git add .
   git commit -m "feat(module): description"
   ```

5. **Push and create PR**
   ```bash
   git push origin feature/my-feature
   # Create PR on GitHub
   ```

---

## Coding Standards

### Rust Style Guide

We follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) and enforce style with `rustfmt`.

**Key Conventions:**

1. **Naming:**
   - Types: `PascalCase`
   - Functions/variables: `snake_case`
   - Constants: `SCREAMING_SNAKE_CASE`
   - Lifetimes: `'a`, `'b`, etc.

2. **Error Handling:**
   - Use `Result<T, E>` for recoverable errors
   - Use `panic!` only for unrecoverable errors
   - Provide context with error types
   - Use `thiserror` for error definitions

3. **Documentation:**
   - All public items must have doc comments
   - Include examples in doc comments
   - Document panics, errors, and safety requirements

4. **Safety:**
   - Minimize `unsafe` code
   - Document all `unsafe` blocks with safety invariants
   - Prefer safe abstractions

### Code Formatting

```bash
# Format all code
cargo fmt

# Check formatting without modifying
cargo fmt -- --check
```

### Linting

```bash
# Run clippy
cargo clippy -- -D warnings

# Run clippy with all features
cargo clippy --all-features -- -D warnings
```

### Example Code Style

```rust
/// Writes a key-value pair to the storage engine.
///
/// # Arguments
///
/// * `key` - The key to write (must be non-empty)
/// * `value` - The value to associate with the key
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if the write fails.
///
/// # Errors
///
/// Returns `StorageError::InvalidKey` if the key is empty.
/// Returns `StorageError::IoError` if the underlying I/O operation fails.
///
/// # Examples
///
/// ```
/// use nanograph::Storage;
///
/// let mut storage = Storage::new()?;
/// storage.put(b"key", b"value")?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn put(&mut self, key: &[u8], value: &[u8]) -> Result<(), StorageError> {
    if key.is_empty() {
        return Err(StorageError::InvalidKey);
    }
    
    self.write_to_wal(key, value)?;
    self.memtable.insert(key.to_vec(), value.to_vec());
    
    Ok(())
}
```

---

## Testing Guidelines

### Test Organization

```
src/
├── lib.rs
├── module.rs
└── module/
    ├── mod.rs
    ├── implementation.rs
    └── tests.rs          # Unit tests
tests/
├── integration_test.rs   # Integration tests
└── common/
    └── mod.rs            # Test utilities
benches/
└── benchmark.rs          # Benchmarks
```

### Unit Tests

- Test each function in isolation
- Cover edge cases and error paths
- Use descriptive test names

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_put_and_get_success() {
        let mut storage = Storage::new().unwrap();
        storage.put(b"key", b"value").unwrap();
        assert_eq!(storage.get(b"key").unwrap(), Some(b"value".to_vec()));
    }

    #[test]
    fn test_put_empty_key_returns_error() {
        let mut storage = Storage::new().unwrap();
        assert!(storage.put(b"", b"value").is_err());
    }

    #[test]
    fn test_get_nonexistent_key_returns_none() {
        let storage = Storage::new().unwrap();
        assert_eq!(storage.get(b"nonexistent").unwrap(), None);
    }
}
```

### Integration Tests

- Test multi-component interactions
- Test end-to-end workflows
- Use realistic scenarios

```rust
// tests/integration_test.rs
use nanograph::{Storage, Config};

#[test]
fn test_persistence_across_restarts() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config = Config::new(temp_dir.path());
    
    // Write data
    {
        let mut storage = Storage::open(config.clone()).unwrap();
        storage.put(b"key", b"value").unwrap();
    }
    
    // Reopen and verify
    {
        let storage = Storage::open(config).unwrap();
        assert_eq!(storage.get(b"key").unwrap(), Some(b"value".to_vec()));
    }
}
```

### Property-Based Tests

Use `proptest` for property-based testing:

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_put_get_roundtrip(key in prop::collection::vec(any::<u8>(), 1..100),
                              value in prop::collection::vec(any::<u8>(), 0..1000)) {
        let mut storage = Storage::new().unwrap();
        storage.put(&key, &value).unwrap();
        prop_assert_eq!(storage.get(&key).unwrap(), Some(value));
    }
}
```

### Benchmarks

Use `criterion` for benchmarks:

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_put(c: &mut Criterion) {
    let mut storage = Storage::new().unwrap();
    
    c.bench_function("put 1KB value", |b| {
        b.iter(|| {
            storage.put(black_box(b"key"), black_box(&[0u8; 1024])).unwrap();
        });
    });
}

criterion_group!(benches, benchmark_put);
criterion_main!(benches);
```

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run with output
cargo test -- --nocapture

# Run benchmarks
cargo bench

# Run with coverage (requires tarpaulin)
cargo tarpaulin --out Html
```

### Test Coverage Goals

- **Unit tests:** 90%+ coverage
- **Integration tests:** All major workflows
- **Property tests:** Core algorithms
- **Benchmarks:** Performance-critical paths

---

## Documentation

### Documentation Types

1. **Code Documentation:** Inline doc comments
2. **ADRs:** Architecture Decision Records (docs/ADR/)
3. **Guides:** User and developer guides (docs/)
4. **API Reference:** Generated from doc comments

### Writing Doc Comments

```rust
/// Brief one-line summary.
///
/// More detailed description with multiple paragraphs if needed.
/// Explain the purpose, behavior, and any important details.
///
/// # Arguments
///
/// * `param1` - Description of first parameter
/// * `param2` - Description of second parameter
///
/// # Returns
///
/// Description of return value.
///
/// # Errors
///
/// List possible error conditions.
///
/// # Panics
///
/// Describe panic conditions if any.
///
/// # Safety
///
/// Document safety requirements for unsafe functions.
///
/// # Examples
///
/// ```
/// use nanograph::example;
///
/// let result = example(42)?;
/// assert_eq!(result, 84);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn example(param: i32) -> Result<i32, Error> {
    // Implementation
}
```

### Generating Documentation

```bash
# Generate and open documentation
cargo doc --open

# Generate with private items
cargo doc --document-private-items
```

### Creating ADRs

When making significant architectural decisions:

1. Copy `docs/ADR/ADR-0001-ADR-Template.md`
2. Number sequentially (ADR-XXXX)
3. Fill in all sections
4. Update `docs/ADR/ADR-0000-Index-of-ADRs.md`
5. Submit as part of your PR

---

## Pull Request Process

### Before Submitting

- [ ] Code compiles without warnings
- [ ] All tests pass
- [ ] Code is formatted (`cargo fmt`)
- [ ] Clippy passes (`cargo clippy`)
- [ ] Documentation is updated
- [ ] Commit messages follow conventions
- [ ] Branch is up to date with develop

### PR Template

```markdown
## Description

Brief description of changes.

## Type of Change

- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation update

## Related Issues

Closes #123

## Testing

Describe testing performed:
- Unit tests added/updated
- Integration tests added/updated
- Manual testing performed

## Checklist

- [ ] Code follows style guidelines
- [ ] Self-review completed
- [ ] Documentation updated
- [ ] Tests added/updated
- [ ] All tests pass
- [ ] No new warnings
```

### Review Process

1. **Automated Checks:** CI must pass
2. **Code Review:** At least one approval required
3. **Testing:** Reviewer verifies tests are adequate
4. **Documentation:** Reviewer checks docs are updated
5. **Merge:** Squash and merge to develop

### Review Guidelines

**For Authors:**
- Keep PRs focused and reasonably sized
- Respond to feedback promptly
- Be open to suggestions
- Update PR based on feedback

**For Reviewers:**
- Review within 2 business days
- Be constructive and specific
- Ask questions if unclear
- Approve when satisfied

---

## Release Process

### Version Numbers

We follow [Semantic Versioning](https://semver.org/):

- **MAJOR:** Breaking changes
- **MINOR:** New features (backward compatible)
- **PATCH:** Bug fixes (backward compatible)

### Release Checklist

1. Update version in `Cargo.toml`
2. Update `CHANGELOG.md`
3. Run full test suite
4. Create release branch
5. Tag release
6. Publish to crates.io
7. Create GitHub release
8. Update documentation site

---

## Getting Help

### Resources

- **Documentation:** [docs/](docs/)
- **ADRs:** [docs/ADR/](docs/ADR/)
- **Glossary:** [docs/GLOSSARY.md](docs/GLOSSARY.md)
- **Implementation Plan:** [docs/DEV/IMPLEMENTATION_PLAN.md](docs/DEV/IMPLEMENTATION_PLAN.md)

### Communication Channels

- **GitHub Issues:** Bug reports and feature requests
- **GitHub Discussions:** Questions and general discussion
- **Pull Requests:** Code review and collaboration

### Asking Questions

When asking for help:

1. Search existing issues/discussions first
2. Provide context and details
3. Include code examples if relevant
4. Describe what you've tried
5. Be patient and respectful

---

## Additional Guidelines

### Performance Considerations

- Profile before optimizing
- Document performance-critical code
- Add benchmarks for hot paths
- Consider memory allocation patterns
- Use appropriate data structures

### Security Considerations

- Validate all inputs
- Handle errors securely
- Avoid information leaks
- Document security assumptions
- Report security issues privately

### Backward Compatibility

- Maintain API stability
- Deprecate before removing
- Provide migration guides
- Version data formats
- Test upgrade paths

---

## Recognition

Contributors are recognized in:

- Git commit history
- Release notes
- Project README
- Annual contributor list

Thank you for contributing to Nanograph! 🚀

---

## License

By contributing to Nanograph, you agree that your contributions will be licensed under the same license as the project (see [LICENSE.md](LICENSE.md)).