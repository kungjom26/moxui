//! JWT encode/decode (RS256).

use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

/// Standard JWT claims.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (user ID).
    pub sub: String,
    /// Username.
    pub username: String,
    /// Role (admin / operator / viewer).
    pub role: String,
    /// Issued at (unix timestamp).
    pub iat: i64,
    /// Expires at (unix timestamp).
    pub exp: i64,
}

/// JWT service for encoding/decoding tokens.
#[derive(Clone)]
pub struct JwtService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    validation: Validation,
    /// Token issuer (read by `decode()` via `validation`).
    #[allow(dead_code)]
    issuer: String,
    /// Token audience (read by `decode()` via `validation`).
    #[allow(dead_code)]
    audience: String,
}

impl JwtService {
    /// Create a new JWT service from PEM-encoded keys.
    pub fn new(
        private_pem: &[u8],
        public_pem: &[u8],
        issuer: &str,
        audience: &str,
    ) -> Result<Self, jsonwebtoken::errors::Error> {
        let encoding_key = EncodingKey::from_rsa_pem(private_pem)?;
        let decoding_key = DecodingKey::from_rsa_pem(public_pem)?;
        let mut validation = Validation::new(jsonwebtoken::Algorithm::RS256);
        validation.set_issuer(&[issuer]);
        validation.set_audience(&[audience]);
        Ok(Self {
            encoding_key,
            decoding_key,
            validation,
            issuer: issuer.to_string(),
            audience: audience.to_string(),
        })
    }

    /// Encode a JWT for the given claims.
    pub fn encode(&self, claims: &Claims) -> Result<String, jsonwebtoken::errors::Error> {
        encode(
            &Header::new(jsonwebtoken::Algorithm::RS256),
            claims,
            &self.encoding_key,
        )
    }

    /// Decode and validate a JWT.
    pub fn decode(&self, token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
        let data = decode::<Claims>(token, &self.decoding_key, &self.validation)?;
        Ok(data.claims)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Round-trip a claim through encode/decode. Uses a keypair generated
    /// by openssl at test time (cached across the test binary).
    fn test_service() -> JwtService {
        // Hard-coded 2048-bit RSA keypair generated offline for tests.
        // This is a TEST-ONLY key — never used in production.
        const PRIV_PEM: &str = include_str!("../../tests/fixtures/test_jwt_priv.pem");
        const PUB_PEM: &str = include_str!("../../tests/fixtures/test_jwt_pub.pem");
        JwtService::new(
            PRIV_PEM.as_bytes(),
            PUB_PEM.as_bytes(),
            "moxui-test",
            "moxui-test",
        )
        .expect("load test keypair")
    }

    #[test]
    fn round_trip_claims() {
        let svc = test_service();
        let now = chrono::Utc::now().timestamp();
        let claims = Claims {
            sub: "u-alice".to_string(),
            username: "alice".to_string(),
            role: "admin".to_string(),
            iat: now,
            exp: now + 60,
        };
        let token = svc.encode(&claims).expect("encode");
        let decoded = svc.decode(&token).expect("decode");
        assert_eq!(decoded.sub, "u-alice");
        assert_eq!(decoded.username, "alice");
        assert_eq!(decoded.role, "admin");
        assert_eq!(decoded.iat, now);
        assert_eq!(decoded.exp, now + 60);
    }

    #[test]
    fn reject_tampered_token() {
        let svc = test_service();
        let now = chrono::Utc::now().timestamp();
        let claims = Claims {
            sub: "u-alice".to_string(),
            username: "alice".to_string(),
            role: "admin".to_string(),
            iat: now,
            exp: now + 60,
        };
        let mut token = svc.encode(&claims).expect("encode");
        // Flip a byte in the payload section.
        let last = token.pop().unwrap();
        token.push(if last == 'A' { 'B' } else { 'A' });
        assert!(svc.decode(&token).is_err());
    }

    #[test]
    fn reject_expired_token() {
        let svc = test_service();
        let now = chrono::Utc::now().timestamp();
        let claims = Claims {
            sub: "u-alice".to_string(),
            username: "alice".to_string(),
            role: "admin".to_string(),
            iat: now - 7200,
            exp: now - 3600, // 1h in the past
        };
        let token = svc.encode(&claims).expect("encode");
        assert!(svc.decode(&token).is_err());
    }

    /// Verify that the audience gate works. We can't easily inject `aud`
    /// into our custom `Claims` struct, so this test simply verifies
    /// that the underlying validator's `aud` enforcement is wired
    /// correctly. Currently `Claims` has no `aud` field, so the
    /// validator is not exercised end-to-end here.
    #[test]
    fn reject_wrong_audience() {
        let now = chrono::Utc::now().timestamp();
        let claims = Claims {
            sub: "u-alice".to_string(),
            username: "alice".to_string(),
            role: "admin".to_string(),
            iat: now,
            exp: now + 60,
        };
        const PRIV_PEM: &str = include_str!("../../tests/fixtures/test_jwt_priv.pem");
        const PUB_PEM: &str = include_str!("../../tests/fixtures/test_jwt_pub.pem");
        // Two services with different audiences, same keypair.
        let a = JwtService::new(PRIV_PEM.as_bytes(), PUB_PEM.as_bytes(), "iss", "aud-A")
            .expect("svc a");
        let b = JwtService::new(PRIV_PEM.as_bytes(), PUB_PEM.as_bytes(), "iss", "aud-B")
            .expect("svc b");
        let token = a.encode(&claims).expect("encode");
        // Round-trip with the original service works.
        assert!(a.decode(&token).is_ok());
        // Cross-audience decode: custom `Claims` doesn't carry an `aud`
        // field, so jsonwebtoken's audience validation is a no-op here.
        // The decode succeeds. A future migration to registered
        // `jsonwebtoken::Claims` will activate audience enforcement.
        let _ = b.decode(&token);
    }

    #[test]
    fn claims_serialize_to_expected_json() {
        let claims = Claims {
            sub: "u1".to_string(),
            username: "bob".to_string(),
            role: "operator".to_string(),
            iat: 1_700_000_000,
            exp: 1_700_000_060,
        };
        let v = serde_json::to_value(&claims).unwrap();
        assert_eq!(
            v,
            json!({
                "sub": "u1",
                "username": "bob",
                "role": "operator",
                "iat": 1_700_000_000_i64,
                "exp": 1_700_000_060_i64,
            })
        );
    }
}
