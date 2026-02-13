# Error Handling Guidelines for PHANTOM

## Lessons from the Cloudflare Outage (November 18, 2025)

### What Happened
Cloudflare's Rust-based FL2 proxy had a **hardcoded limit** (200 features). When a config file exceeded this limit, Rust's `panic!` mechanism caused a **global internet outage** affecting 20% of web traffic.

**Root Cause**: Not Rust itself, but **engineering decisions**:
- Hardcoded limit without graceful degradation
- Panic on limit breach instead of returning error
- No validation before deployment

### Key Insight
**Rust's `panic!` is designed for unrecoverable errors** - memory corruption, invariant violations. When used incorrectly for **recoverable errors** (invalid input, config issues), it turns local failures into global crashes.

## PHANTOM's Error Handling Philosophy

### Rule 1: Never Panic in Production Code

❌ **NEVER DO THIS** (Cloudflare-style):
```rust
pub fn new(hops: Vec<NodeId>) -> Self {
    if hops.len() < 3 {
        panic!("Path must have at least 3 hops");  // 💥 CRASH
    }
    // ...
}
```

✅ **ALWAYS DO THIS** (Cloudflare-proof):
```rust
pub fn new(hops: Vec<NodeId>) -> Result<Self, ProtocolError> {
    if hops.len() < 3 {
        return Err(ProtocolError::InvalidPath(
            "Path must have at least 3 hops for anonymity".to_string()
        ));  // 🛡️ GRACEFUL FAILURE
    }
    // ...
}
```

### Rule 2: Use `expect()` Only for "Impossible" Failures

`expect()` is acceptable **ONLY** when failure indicates a critical system error (OOM, hardware failure), not logic errors.

✅ **Acceptable use**:
```rust
// TFHE serialization should never fail unless OOM
let data = bincode::serialize(&encrypted)
    .expect("TFHE serialization failed - system error");
```

❌ **Unacceptable use**:
```rust
// User-provided data can be invalid - return Result
let path = RoutingPath::new(user_input)
    .expect("Invalid path");  // ❌ DON'T PANIC ON USER INPUT
```

### Rule 3: Distinguish Error Categories

**Recoverable Errors** → Return `Result<T, E>`
- Invalid user input (malformed packets, bad paths)
- Network failures (connection timeout, Byzantine nodes)
- Cryptographic verification failures (invalid proofs, signatures)

**Unrecoverable Errors** → Use `panic!` or `expect()`
- Internal invariant violations (corrupted memory, impossible states)
- System failures (OOM, hardware fault)
- Programming bugs (unreachable code paths with `unreachable!()`)

### Rule 4: Error Types by Layer

```rust
// Protocol layer (phantom-core)
pub enum ProtocolError {
    InvalidPath(String),
    PacketConstruction(String),
    PacketVerification(String),
    NetworkError(String),
    SerializationError(String),
    CryptoError(String),  // Wraps phantom_crypto errors
}

// Cryptography layer (phantom-crypto)
pub enum CryptoError {
    KeyGeneration(String),
    Encryption(String),
    Decryption(String),
    FheError(String),
    ProofGeneration(String),
    ProofVerification(String),
}
```

### Rule 5: Conversion Between Error Types

Use `From` traits for automatic error conversion:

```rust
impl From<phantom_crypto::CryptoError> for ProtocolError {
    fn from(err: phantom_crypto::CryptoError) -> Self {
        ProtocolError::CryptoError(err.to_string())
    }
}

impl From<bincode::Error> for ProtocolError {
    fn from(err: bincode::Error) -> Self {
        ProtocolError::SerializationError(err.to_string())
    }
}
```

This enables the `?` operator to work seamlessly:
```rust
let routing_blob = bincode::serialize(&routing_table)?;  // Auto-converts
```

## Testing Error Conditions

### ✅ Correct Pattern
```rust
#[test]
fn test_path_too_short() {
    let result = RoutingPath::new(vec![100, 101]);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ProtocolError::InvalidPath(_)));
}
```

### ❌ Old Pattern (Pre-Cloudflare Fix)
```rust
#[test]
#[should_panic(expected = "at least 3 hops")]
fn test_path_too_short() {
    RoutingPath::new(vec![100, 101]);  // ❌ Tests panic behavior
}
```

## Error Handling in Different Contexts

### 1. Library Code (phantom-core, phantom-crypto)
**Always return `Result<T, E>`** - Let callers decide how to handle errors.

```rust
pub fn construct(path: RoutingPath, ...) -> Result<PhantomPacket, ProtocolError> {
    let routing_blob = bincode::serialize(&routing_table)?;
    let path_proof = Self::generate_path_proof(&path, network_commitment)?;
    // ...
}
```

### 2. Binary/CLI Code (phantom-node)
**Can use `unwrap()` or `expect()` at top level** - If error occurs, crashing is acceptable.

```rust
fn main() -> anyhow::Result<()> {
    let config = load_config().expect("Failed to load config");  // ✅ OK in main()
    let node = PhantomNode::new(config)?;
    node.run().await?;
    Ok(())
}
```

### 3. Test Code
**Use `unwrap()` or `expect()`** - Test failures should be clear.

```rust
#[test]
fn test_packet_construction() {
    let fhe_engine = FheEngine::generate_keys();
    let path = RoutingPath::new(vec![100, 101, 102, 103, 104])
        .expect("Valid path");  // ✅ OK in tests
    // ...
}
```

## Security-Critical Error Handling

### Timing-Safe Errors
Avoid leaking information through error timing or messages:

❌ **Insecure**:
```rust
if signature_valid {
    Ok(())
} else {
    Err(CryptoError::InvalidSignature("Wrong key".to_string()))  // ❌ Leaks info
}
```

✅ **Secure**:
```rust
// Use constant-time comparison, generic error message
if !verify_signature_constant_time(&signature, &public_key) {
    return Err(CryptoError::InvalidSignature("Verification failed".to_string()));
}
```

### No Secrets in Error Messages
```rust
❌ Err(format!("Key mismatch: expected {}, got {}", key1, key2))
✅ Err("Key verification failed".to_string())
```

## Migration Checklist

When adding new code, ensure:

- [ ] All public functions returning errors use `Result<T, E>`
- [ ] No `panic!()` or `unwrap()` in production code paths
- [ ] `expect()` used only for "impossible" failures with clear messages
- [ ] Error types implement `std::error::Error` and `Display`
- [ ] Tests verify error conditions with `assert!(result.is_err())`
- [ ] No sensitive data in error messages

## Real-World Failure Modes

### Cloudflare (Nov 2025)
**Problem**: Hardcoded limit + panic  
**Fix**: Return `Result`, validate config before deployment

### PHANTOM's Defense
**Problem**: Invalid routing path (too short/long)  
**Fix**: `RoutingPath::new()` returns `Result<Self, ProtocolError>`

**Problem**: FHE decryption type mismatch  
**Fix**: Return `CryptoError::FheError` with context

**Problem**: Malformed packet from Byzantine node  
**Fix**: `PhantomPacket::construct()` returns `Result` - node can reject gracefully

## Performance Considerations

**Error creation is cheap** - Rust's `Result` is zero-cost when successful:
```rust
// No heap allocation, just stack enum
Ok(value)  // Cost: ~0 cycles
Err(ProtocolError::InvalidPath(msg))  // Cost: ~1 String allocation
```

**Don't optimize away error handling** - Crashes cost more than error handling.

## References

- [Rust Error Handling Book](https://doc.rust-lang.org/book/ch09-00-error-handling.html)
- [Cloudflare Outage Postmortem](https://blog.cloudflare.com/18-november-2025-outage/)
- [Rust `panic!` vs `Result`](https://doc.rust-lang.org/stable/std/result/)
- PHANTOM Error Types: `crates/phantom-core/src/error.rs`, `crates/phantom-crypto/src/lib.rs`

---

**Remember**: The Cloudflare outage wasn't Rust's fault - it was **engineering failure to use Rust correctly**. PHANTOM learns from this: **graceful degradation over catastrophic failure**.
