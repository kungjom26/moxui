//! Password hashing (bcrypt cost 12).

use bcrypt::{hash, verify, BcryptError};

/// Hash a password using bcrypt with cost factor 12.
///
/// # Errors
///
/// Returns error if bcrypt hashing fails (e.g., invalid password length).
pub fn hash_password(password: &str) -> Result<String, BcryptError> {
    hash(password, 12)
}

/// Verify a password against a bcrypt hash.
///
/// Returns `Ok(true)` if match, `Ok(false)` if mismatch.
pub fn verify_password(password: &str, hash: &str) -> Result<bool, BcryptError> {
    verify(password, hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_and_verify() {
        let password = "super-secret-password-123";
        let hashed = hash_password(password).unwrap();
        assert!(verify_password(password, &hashed).unwrap());
        assert!(!verify_password("wrong-password", &hashed).unwrap());
    }

    #[test]
    fn test_hash_produces_different_output() {
        // bcrypt uses random salt, so same password produces different hashes
        let password = "test";
        let hash1 = hash_password(password).unwrap();
        let hash2 = hash_password(password).unwrap();
        assert_ne!(hash1, hash2);
        // But both verify correctly
        assert!(verify_password(password, &hash1).unwrap());
        assert!(verify_password(password, &hash2).unwrap());
    }
}
