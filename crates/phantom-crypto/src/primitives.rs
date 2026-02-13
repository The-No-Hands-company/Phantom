//! Low-level cryptographic primitives

use blake3;

/// BLAKE3 hash function (cryptographic hash)
pub fn hash(data: &[u8]) -> [u8; 32] {
    blake3::hash(data).into()
}

/// Derive a key from a shared secret using BLAKE3
pub fn derive_key(shared_secret: &[u8], context: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(shared_secret);
    hasher.update(context);
    hasher.finalize().into()
}

/// Constant-time comparison
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash() {
        let data = b"PHANTOM protocol";
        let h1 = hash(data);
        let h2 = hash(data);
        assert_eq!(h1, h2);
        
        let different = b"Different data";
        let h3 = hash(different);
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_constant_time_eq() {
        let a = [1, 2, 3, 4];
        let b = [1, 2, 3, 4];
        let c = [1, 2, 3, 5];
        
        assert!(constant_time_eq(&a, &b));
        assert!(!constant_time_eq(&a, &c));
    }
}
