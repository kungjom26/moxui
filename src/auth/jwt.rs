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

    // TODO: generate test keypair, test round-trip
}
