# Compression and Encryption Implementation Plan for nanograph-wal

## Status: NOT IMPLEMENTED (False Advertising)

The README and documentation advertise compression and encryption support, but these features are **not implemented**. This document outlines the complete implementation plan.

## Current State

### What Exists
- ✅ Configuration fields in `WriteAheadLogConfig`:
  - `compression: CompressionAlgorithm`
  - `encryption: EncryptionAlgorithm`
  - `encryption_key: Option<EncryptionKey>`
- ✅ Segment header includes compression/encryption algorithm fields
- ✅ Full compression/encryption support in `nanograph-util` crate
- ❌ **NO actual compression or encryption of WAL records**
- ❌ **NO decompression or decryption when reading**

### What's Missing
1. Compression/encryption logic in `WriteAheadLogFile::append()`
2. Decompression/decryption logic in `WriteAheadLogReader::next()`
3. Encryption key and nonce management
4. Modified record format to support metadata
5. Tests for compressed/encrypted records
6. Documentation updates

## Implementation Plan

### Phase 1: Record Format Changes (2-3 days)

#### Current Record Format
```
| Magic (4) | Kind (2) | Len (4) | Payload (len) | Checksum (4) |
```

#### Proposed Record Format (with compression/encryption)
```
| Magic (4) | Version (2) | Kind (2) | Flags (1) | OrigLen (4) | CompLen (4) | Nonce (12) | Payload (complen) | Checksum (4) |
```

**Flags byte:**
- Bit 0: Compressed (0=no, 1=yes)
- Bit 1: Encrypted (0=no, 1=yes)
- Bits 2-7: Reserved

**Fields:**
- `OrigLen`: Original payload length (before compression)
- `CompLen`: Compressed/encrypted payload length
- `Nonce`: 12-byte nonce for encryption (only if encrypted)

#### Changes Required
1. Update `RECORD_MAGIC` or add version field
2. Modify `WriteAheadLogFile::append()` to write new format
3. Modify `WriteAheadLogReader::next()` to read new format
4. Maintain backward compatibility with old format (version check)

### Phase 2: Compression Implementation (3-4 days)

#### Writer Side (`WriteAheadLogFile`)
```rust
fn append(&mut self, record: &WriteAheadLogRecord) -> WriteAheadLogResult<LogSequenceNumber> {
    let original_len = record.payload.len();
    
    // Compress if enabled
    let (payload, compressed) = if self.compression != CompressionAlgorithm::None {
        let compressed = self.compression.compress(record.payload)?;
        // Only use compression if it actually reduces size
        if compressed.len() < original_len {
            (compressed, true)
        } else {
            (record.payload.to_vec(), false)
        }
    } else {
        (record.payload.to_vec(), false)
    };
    
    // Write record with compression flag
    // ...
}
```

#### Reader Side (`WriteAheadLogReader`)
```rust
pub fn next(&mut self) -> WriteAheadLogResult<Option<WriteAheadLogEntry>> {
    // Read header with flags
    // ...
    
    // Decompress if needed
    let payload = if flags & 0x01 != 0 {
        self.compression.decompress(&raw_payload, Some(original_len))?
    } else {
        raw_payload
    };
    
    // Return entry
    // ...
}
```

#### Testing
- Test each compression algorithm (None, Lz4, Zstd, Snappy)
- Test compression ratio tracking
- Test fallback when compression doesn't help
- Test decompression errors

### Phase 3: Encryption Implementation (5-7 days)

#### Key Management Strategy

**Option A: Per-Segment Key (Simpler)**
- Store encryption key in `WriteAheadLogFile` struct
- Generate one nonce per record
- Store nonce in record header

**Option B: Per-Record Key Derivation (More Secure)**
- Derive per-record key from master key + LSN
- Generate nonce per record
- More complex but better security

**Recommendation: Start with Option A**

#### Writer Side
```rust
struct WriteAheadLogFile {
    // ... existing fields ...
    encryption_key: Option<EncryptionKey>,
    encryption: EncryptionAlgorithm,
}

fn append(&mut self, record: &WriteAheadLogRecord) -> WriteAheadLogResult<LogSequenceNumber> {
    // ... compression logic ...
    
    // Encrypt if enabled
    let (payload, nonce) = if self.encryption != EncryptionAlgorithm::None {
        let key = self.encryption_key.as_ref()
            .ok_or(WriteAheadLogError::MissingEncryptionKey)?;
        let nonce = self.encryption.generate_nonce();
        let encrypted = self.encryption.encrypt(key, &nonce, &payload)?;
        (encrypted, Some(nonce))
    } else {
        (payload, None)
    };
    
    // Write record with encryption flag and nonce
    // ...
}
```

#### Reader Side
```rust
struct WriteAheadLogReader {
    // ... existing fields ...
    encryption_key: Option<EncryptionKey>,
    encryption: EncryptionAlgorithm,
}

pub fn next(&mut self) -> WriteAheadLogResult<Option<WriteAheadLogEntry>> {
    // ... read header and decompress ...
    
    // Decrypt if needed
    let payload = if flags & 0x02 != 0 {
        let key = self.encryption_key.as_ref()
            .ok_or(WriteAheadLogError::MissingEncryptionKey)?;
        let nonce = Nonce::from_bytes(self.encryption, nonce_bytes)?;
        self.encryption.decrypt(key, &nonce, &payload)?
    } else {
        payload
    };
    
    // Return entry
    // ...
}
```

#### Key Management
1. Add `encryption_key` field to `WriteAheadLogFile`
2. Pass key from `WriteAheadLogManager` during segment creation
3. Store key securely (not in segment file!)
4. Add key rotation support (future enhancement)

#### Testing
- Test each encryption algorithm (None, Aes256Gcm, ChaCha20Poly1305)
- Test encryption/decryption round-trip
- Test wrong key detection
- Test nonce uniqueness
- Test performance impact

### Phase 4: Integration (2-3 days)

#### Manager Changes
```rust
impl WriteAheadLogManager {
    pub fn new(fs, directory, config) -> WriteAheadLogResult<Self> {
        // Validate encryption config
        if config.encryption != EncryptionAlgorithm::None {
            if config.encryption_key.is_none() {
                return Err(WriteAheadLogError::MissingEncryptionKey);
            }
        }
        
        // Pass encryption key to segment
        let active_segment = WriteAheadLogFile::create(
            file,
            config.shard_id,
            0,
            0,
            config.checksum,
            config.compression,
            config.encryption,
            config.encryption_key.clone(),
        )?;
        
        // ...
    }
}
```

#### Error Handling
Add new error variants:
```rust
pub enum WriteAheadLogError {
    // ... existing variants ...
    MissingEncryptionKey,
    CompressionFailed(String),
    DecompressionFailed(String),
    EncryptionFailed(String),
    DecryptionFailed(String),
}
```

### Phase 5: Testing (3-4 days)

#### Unit Tests
- Compression only
- Encryption only
- Compression + Encryption
- Each algorithm combination
- Error cases (wrong key, corrupted data)
- Performance benchmarks

#### Integration Tests
- Write compressed, read back
- Write encrypted, read back
- Write compressed+encrypted, read back
- Segment rotation with compression/encryption
- Recovery with compression/encryption

#### Example Programs
Update examples to demonstrate:
- Compression configuration
- Encryption configuration
- Combined usage

### Phase 6: Documentation (1-2 days)

#### Update Documentation
1. README.md - Accurate feature description
2. lib.rs - Update examples to show actual usage
3. Security considerations for encryption
4. Performance impact of compression/encryption
5. Key management best practices

#### Remove False Advertising
- Mark unimplemented features as "Planned" or "Future"
- Or implement them fully

## Estimated Timeline

- **Phase 1 (Record Format)**: 2-3 days
- **Phase 2 (Compression)**: 3-4 days
- **Phase 3 (Encryption)**: 5-7 days
- **Phase 4 (Integration)**: 2-3 days
- **Phase 5 (Testing)**: 3-4 days
- **Phase 6 (Documentation)**: 1-2 days

**Total: 16-23 days (3-4 weeks)**

## Alternative: Remove False Advertising (1 day)

If full implementation is not feasible:

1. Remove compression/encryption examples from lib.rs
2. Update README to mark as "Planned" or "Future"
3. Add TODO comments in code
4. Update issue to track future implementation

## Dependencies

- ✅ nanograph-util with compression/encryption support
- ✅ Segment header format supports algorithm fields
- ❌ Record format needs modification
- ❌ Key management infrastructure

## Security Considerations

### Encryption
- Never log encryption keys
- Use secure key storage (not in segment files)
- Implement key rotation
- Use authenticated encryption (AES-GCM, ChaCha20-Poly1305)
- Ensure nonce uniqueness

### Compression
- Compression before encryption (standard practice)
- Be aware of compression ratio attacks
- Consider disabling compression for encrypted data in some cases

## Performance Considerations

### Compression
- LZ4: Fast, moderate compression
- Zstd: Slower, better compression
- Snappy: Very fast, light compression
- Overhead: 10-30% CPU, 30-70% size reduction

### Encryption
- AES-256-GCM: Hardware accelerated on modern CPUs
- ChaCha20-Poly1305: Fast software implementation
- Overhead: 5-15% CPU, 16 bytes per record (auth tag)

## Backward Compatibility

### Reading Old Format
- Check magic number or version field
- Fall back to old format reader if needed
- Maintain old format support for 2-3 major versions

### Writing New Format
- Always write new format once implemented
- Provide migration tool for old segments

## References

- nanograph-util compression: `nanograph-util/src/compression.rs`
- nanograph-util encryption: `nanograph-util/src/encryption.rs`
- Current WAL format: `nanograph-wal/src/walfile.rs`
- Issue: nanograph-3ge, nanograph-emh, nanograph-kfa