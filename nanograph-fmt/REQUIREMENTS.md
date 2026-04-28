# Nanograph-fmt `Data eXchange Format (DXF)` Requirements Specification

## Overview

The `nanograph-fmt` crate provides two complementary table schema formats for the Nanograph database system:

1. **Dynamic Record Format (DRF)** - A self-describing, schema-flexible format similar to JSON/RON
2. **Static Record Format (SRF)** - A schema-defined, type-safe format similar to Apache Avro

Both formats support both binary and textual representations, enabling efficient storage and human-readable debugging.

---

## 1. Dynamic Record Format (DRF)

### 1.1 Purpose

DRF provides a flexible, self-describing data format where each record contains its own type information. This is ideal for:
- Schema evolution and migration
- Heterogeneous data collections
- Rapid prototyping and development
- Human-readable data inspection

### 1.2 Format Characteristics

#### 1.2.1 Self-Describing
- Each value includes type metadata
- No external schema required for deserialization
- Supports arbitrary nesting and composition

#### 1.2.2 Dual Representation
- **Textual Format**: RON-like syntax for human readability
- **Binary Format**: Compact binary encoding for storage efficiency

#### 1.2.3 Optional Schema Format
- Describe expected fields, optional type constraints, and validation rules for records
- Support for default values and custom validation logic
- Schema validation and enforcement during deserialization

### 1.3 Supported Types

#### 1.3.1 Primitive Types
- **Numbers**: `42`, `3.14`, `0b1010`, `0o755`, `0xdeadbeef`
- **Boolean**: `true`, `false`
- **String**: UTF-8 encoded strings `"Hello"`, `"with\\escapes\n"`, `r#"raw string, great for regex\."#`
- **Bytes**: Raw byte arrays `b"with \x65\x66\x67"`, `br#"raw, too"#`
- **Chars**: Single Unicode characters `'a'`, `'\u{1F600}'`
- **Null**: Explicit null/none value

#### 1.3.2 Composite Types
- **List**: Ordered sequence of values (homogeneous or heterogeneous) `["list", "of", 3.14]`
- **Map**: Key-value pairs with arbitrary keys and values `{"key": "value", 1: 2}`
- **Struct**: Named field collections `( foo: 1.0, bar: ( baz: "I'm nested" ) )`
- **Tuple**: Fixed-size ordered collections `("abc", 1.23, true)`, `()`
- **Tagged Union**: Discriminated unions with variant names

#### 1.3.3 Advanced Types

- **Timestamp**: Date/time values with timezone support
- **UUID**: Universally unique identifiers
- **Decimal**: Arbitrary precision decimal numbers
- **Reference**: Pointer-like type for cross-referencing data

### 1.4 Textual Format Syntax

#### 1.4.1 Basic Syntax
```ron
// Primitives
42
3.14159
true
"hello world"
null

// Collections
[1, 2, 3, 4, 5]
{"name": "Alice", "age": 30}

// Structs
Person(name: "Bob", age: 25, active: true)

// Tuples
(1, "two", 3.0)

// Tagged Unions
Result::Ok(42)
Result::Err("error message")

// Options
Some(100)
None
```

#### 1.4.2 Nested Structures
```ron
User(
    id: 12345,
    profile: Profile(
        name: "Alice",
        email: "alice@example.com",
        tags: ["admin", "developer"]
    ),
    settings: {
        "theme": "dark",
        "notifications": true
    }
)
```

### 1.5 Binary Format Specification

#### 1.5.1 Type Tags
- 1 byte type discriminator
- Variable-length encoding for sizes
- Efficient representation of common types

#### 1.5.2 Encoding Rules
- **Integers**: Variable-length encoding (LEB128)
- **Floats**: IEEE 754 binary representation
- **Strings**: Length-prefixed UTF-8
- **Collections**: Length-prefixed with element encoding
- **Structs**: Field count + name/value pairs
- **Tagged Unions**: Variant index + payload

#### 1.5.3 Optimization Strategies
- VarInt based Small integer optimization
- String interning for repeated values
- Shared type descriptors for repeated structures

### 1.6 API Requirements

#### 1.6.1 Serialization
```rust
trait DynamicSerialize {
    fn to_text(&self) -> Result<String, Error> {
        let mut str = String::new();
        Self::to_drf_text_buffer(&mut str);
        Ok(str)
    }
    fn to_text_buffer(&self, out: &mut String) -> Result<(),Error>;
    fn to_binary(&self) -> Result<Vec<u8>, Error> {
        let mut out = Vec::new();
        Self::to_drf_binary_buffer(&mut out);
        Ok(out)
    }
    fn to_binary_buffer(&self, out: &mut Vec<u8>) -> Result<(), Error>;
}
```

#### 1.6.2 Deserialization
```rust
trait DynamicDeserialize: Sized {
    fn from_drf_text(text: &str) -> Result<Self, Error>;
    fn from_drf_binary(bytes: &[u8]) -> Result<Self, Error>;
}
```

#### 1.6.3 Dynamic Value API
```rust
enum DynamicValue {
    Null,
    Bool(bool),
    Number(i64),
    String(String),
    Bytes(Vec<u8>),
    List(Vec<DynamicValue>),
    Tuple(Vec<DynamicValue>),
    Map(HashMap<DynamicValue, DynamicValue>),
    Record(String, HashMap<String, DynamicValue>),
    TaggedUnion(String, Box<DynamicValue>),
}
```

---

## 2. Static Record Format (SRF)

### 2.1 Purpose

SRF provides a schema-defined, type-safe format where the schema is configuration defined. This is ideal for:
- High-performance serialization/deserialization
- Type safety and validation
- Compact storage with minimal overhead
- Interoperability with schema registries

### 2.2 Format Characteristics

#### 2.2.1 Schema-Defined
- External schema definition required
- Type information isn’t stored with data
- Efficient encoding without type tags

#### 2.2.2 Avro-Like Design
- Similar to Apache Avro record format
- Schema evolution support
- Forward and backward compatibility

### 2.3 Schema Definition Language

#### 2.3.1 JSON Schema Syntax
```json
{
  "type": "record",
  "name": "User",
  "fields": [
    {"name": "id", "type": "long"},
    {"name": "username", "type": "string"},
    {"name": "email", "type": "string"},
    {"name": "age", "type": "int"},
    {"name": "active", "type": "boolean"},
    {"name": "tags", "type": {"type": "array", "items": "string"}},
    {"name": "metadata", "type": {"type": "map", "values": "string"}}
  ]
}
```

#### 2.3.2 Alternative RON-Based Schema
```ron
Record(
    name: "User",
    fields: [
        Field(name: "id", type: Long),
        Field(name: "username", type: String),
        Field(name: "email", type: String),
        Field(name: "age", type: Int),
        Field(name: "active", type: Boolean),
        Field(name: "tags", type: Array(String)),
        Field(name: "metadata", type: Map(String, String))
    ]
)
```

### 2.4 Supported Schema Types

#### 2.4.1 Primitive Types
- **Null**: Null value
- **Boolean**: true/false
- **Int**: Signed integer (4, 8, 16, 32, 64, 128)
- **UInt**: Unsigned integer (4, 8, 16, 32, 64, 128)
- **VInt**: Variable Length BigInt Integer
- **Float**: IEEE 754 floating point (16, 32, 64, 128, 256)
- **Bytes**: Arbitrary length byte array
- **Fixed**: Fixed-size byte array

#### 2.4.2 Complex Types
- **Record/Struct**: Named collection of fields
- **Enum**: Enumeration of named values
- **Array**: Ordered collection of items (single type)
- **Map**: Key-value pairs (string keys, typed values)
- **Union**: Tagged union of multiple types
- **Reference**: Record Reference in Same or Another Table

#### 2.4.3 Logical Types
- **Complex**: Complex Numbers
- **String**: UTF-8 encoded text in a bytes
- **Decimal**: Arbitrary precision decimal
- **UUID**: 128-bit universally unique identifier
- **Date**: Date without time
- **Time**: Time of day in milliseconds
- **Timestamp**: Unix timestamp in milliseconds
- **Timestampz**: Unix timestamp in microseconds with timezone
- **Duration**: Time duration

### 2.5 Binary Encoding

#### 2.5.1 Encoding Rules
- **Integers**: Zigzag + variable-length encoding
- **Floats**: IEEE 754 binary representation
- **Strings**: Length-prefixed UTF-8
- **Bytes**: Length-prefixed raw bytes
- **Arrays**: Length-prefixed elements
- **Maps**: Length-prefixed key-value pairs
- **Unions**: Index + value
- **Records**: Concatenated field values (no names)

#### 2.5.2 Optimization
- No field names in binary format
- No type tags (schema provides types)
- Minimal overhead for primitive types
- Efficient variable-length encoding

### 2.6 Schema Evolution

#### 2.6.1 Compatibility Rules
- **Forward Compatibility**: New schema can read old data
- **Backward Compatibility**: Old schema can read new data
- **Full Compatibility**: Both forward and backward compatible

#### 2.6.2 Evolution Operations
- **Add Field**: With default value (forward compatible)
- **Remove Field**: Old readers ignore (backward compatible)
- **Rename Field**: Use aliases
- **Change Type**: Limited promotions (int → long, float → double)

#### 2.6.3 Default Values
```json
{
  "name": "status",
  "type": "string",
  "default": "active"
}
```

### 2.7 API Requirements

#### 2.7.1 Schema Definition
```rust
struct Schema {
    name: String,
    fields: Vec<Field>,
    // ... metadata
}

struct Field {
    name: String,
    field_type: FieldType,
    default: Option<Value>,
    // ... metadata
}
```

#### 2.7.2 Serialization
```rust
trait StaticSerialize {
    fn schema() -> Schema;
    fn serialize(&self, writer: &mut impl Write) -> Result<(), Error>;
}
```

#### 2.7.3 Deserialization
```rust
trait StaticDeserialize: Sized {
    fn schema() -> Schema;
    fn deserialize(reader: &mut impl Read) -> Result<Self, Error>;
}
```

#### 2.7.4 Schema Registry Integration
```rust
trait SchemaRegistry {
    fn register_schema(&mut self, schema: Schema) -> Result<SchemaId, Error>;
    fn get_schema(&self, id: SchemaId) -> Result<Schema, Error>;
    fn check_compatibility(&self, schema: &Schema, id: SchemaId) -> Result<bool, Error>;
}
```

---

## 3. Common Requirements

### 3.1 Performance

#### 3.1.1 Serialization Performance
- DRF: Target < 1μs for simple records
- SRF: Target < 500ns for simple records
- Zero-copy deserialization where possible

#### 3.1.2 Memory Efficiency
- Minimal allocation during serialization
- Streaming support for large records
- Buffer reuse for repeated operations

### 3.2 Error Handling

#### 3.2.1 Error Types
```rust
enum FormatError {
    ParseError(String),
    TypeMismatch { expected: String, found: String },
    SchemaViolation(String),
    IoError(std::io::Error),
    InvalidUtf8(std::string::FromUtf8Error),
    // ... additional error types
}
```

#### 3.2.2 Error Context
- Clear error messages
- Position information for parse errors
- Schema path for validation errors

### 3.3 Validation

#### 3.3.1 DRF Validation
- UTF-8 validation for strings
- Structural validation (balanced brackets, etc.)
- Type consistency in collections

#### 3.3.2 SRF Validation
- Schema conformance checking
- Required field validation
- Type constraint validation
- Range validation for logical types

### 3.4 Testing Requirements

#### 3.4.1 Unit Tests
- All type encodings/decodings
- Round-trip tests (serialize → deserialize)
- Error condition handling
- Edge cases (empty collections, null values, etc.)

#### 3.4.2 Integration Tests
- Cross-format compatibility
- Schema evolution scenarios
- Large dataset handling
- Concurrent access patterns

#### 3.4.3 Property-Based Tests
- Arbitrary data generation
- Round-trip property verification
- Compatibility property verification

#### 3.4.4 Benchmarks
- Serialization performance
- Deserialization performance
- Memory usage profiling
- Comparison with other formats (JSON, MessagePack, Avro)

### 3.5 Documentation

#### 3.5.1 API Documentation
- Comprehensive rustdoc comments
- Usage examples for all public APIs
- Performance characteristics documented

#### 3.5.2 Format Specification
- Detailed binary format specification
- Textual format grammar (EBNF)
- Schema definition language specification

#### 3.5.3 User Guide
- Getting started guide
- Format selection guide (DRF vs SRF)
- Schema evolution best practices
- Performance tuning guide

---

## 4. Implementation Phases

### Phase 1: Core Infrastructure
- [ ] Define error types and result types
- [ ] Implement basic type system
- [ ] Create buffer management utilities
- [ ] Set up testing framework

### Phase 2: Dynamic Record Format (DRF)
- [ ] Implement DRF type system
- [ ] Implement textual parser (RON-like)
- [ ] Implement textual serializer
- [ ] Implement binary encoder
- [ ] Implement binary decoder
- [ ] Add comprehensive tests

### Phase 3: Static Record Format (SRF)
- [ ] Implement schema definition types
- [ ] Implement schema parser (JSON/RON)
- [ ] Implement binary encoder
- [ ] Implement binary decoder
- [ ] Add schema validation
- [ ] Add comprehensive tests

### Phase 4: Advanced Features
- [ ] Schema evolution support
- [ ] Schema registry implementation
- [ ] Optimization passes
- [ ] Streaming support
- [ ] Zero-copy deserialization

### Phase 5: Integration & Polish
- [ ] Derive macros for common types
- [ ] Integration with serde ecosystem
- [ ] Performance benchmarks
- [ ] Documentation completion
- [ ] Examples and tutorials

---

## 5. Dependencies

### 5.1 Required Dependencies
- None (pure Rust implementation preferred)

### 5.2 Optional Dependencies
- `serde` - For serde integration
- `ron` - For RON format compatibility
- `uuid` - For UUID type support
- `chrono` - For timestamp types

### 5.3 Development Dependencies
- `criterion` - For benchmarking
- `proptest` - For property-based testing
- `quickcheck` - Alternative property testing

---

## 6. Non-Functional Requirements

### 6.1 Compatibility
- Rust 1.70+ (or current MSRV)
- Windows, Linux, macOS support
- 32-bit and 64-bit architectures
- no_std support (with alloc)

### 6.2 Security
- No unsafe code in public API
- Bounds checking on all array access
- Protection against malicious input
- Resource exhaustion prevention

### 6.3 Maintainability
- Clear code organization
- Comprehensive test coverage (>90%)
- CI/CD integration
- Semantic versioning

---

## 7. Success Criteria

### 7.1 Functional Criteria
- ✓ All specified types supported in both formats
- ✓ Round-trip serialization works correctly
- ✓ Schema evolution works as specified
- ✓ All tests pass on all platforms

### 7.2 Performance Criteria
- ✓ DRF performance within 2x of JSON
- ✓ SRF performance within 1.5x of MessagePack
- ✓ Binary format smaller than JSON
- ✓ Zero-copy deserialization for applicable types

### 7.3 Quality Criteria
- ✓ Test coverage > 90%
- ✓ No clippy warnings
- ✓ Documentation coverage > 95%
- ✓ All examples compile and run

---

## 8. Future Considerations

### 8.1 Potential Extensions
- Compression support (zstd, lz4)
- Encryption support
- Streaming compression
- Custom logical types
- Schema inference from data

### 8.2 Integration Points
- nanograph-kvt integration
- nanograph-lsm integration
- Network protocol support
- Foreign function interface (FFI)

---

## Appendix A: Format Comparison

| Feature | DRF | SRF | JSON | MessagePack | Avro |
|---------|-----|-----|------|-------------|------|
| Self-describing | ✓ | ✗ | ✓ | ✓ | ✗ |
| Schema required | ✗ | ✓ | ✗ | ✗ | ✓ |
| Binary format | ✓ | ✓ | ✗ | ✓ | ✓ |
| Text format | ✓ | ✗ | ✓ | ✗ | ✗ |
| Schema evolution | Limited | ✓ | N/A | N/A | ✓ |
| Type safety | Runtime | Compile | Runtime | Runtime | Compile |
| Performance | Medium | High | Low | Medium | High |
| Size efficiency | Medium | High | Low | Medium | High |

---

## Appendix B: Example Use Cases

### B.1 DRF Use Case: Configuration Files
```ron
DatabaseConfig(
    host: "localhost",
    port: 5432,
    pools: {
        "read": PoolConfig(size: 10, timeout: 30),
        "write": PoolConfig(size: 5, timeout: 60)
    },
    features: ["replication", "backup", "monitoring"]
)
```

### B.2 SRF Use Case: High-Volume Event Logging
```rust
// Schema defined once
let schema = Schema::new("LogEvent")
    .field("timestamp", Type::TimestampMicros)
    .field("level", Type::Enum(vec!["DEBUG", "INFO", "WARN", "ERROR"]))
    .field("message", Type::String)
    .field("metadata", Type::Map(Type::String));

// Efficient binary serialization
let event = LogEvent { /* ... */ };
event.serialize(&mut writer)?;
```

---

*Document Version: 1.0*  
*Last Updated: 2026-01-28*  
*Status: Draft for Review*