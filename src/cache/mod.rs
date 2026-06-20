//! In-memory cache layer (moka with TTL + LRU).
//!
//! Used for Proxmox API responses (VM list, cluster stats, etc.)
//! to reduce load and improve response times.

use std::time::Duration;

/// Cache configuration.
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Default TTL for cached entries.
    pub default_ttl: Duration,
    /// Maximum number of entries.
    pub max_capacity: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            default_ttl: Duration::from_secs(5),
            max_capacity: 10_000,
        }
    }
}

// TODO: implement moka-based cache wrapper
// See ~/.hermes/profiles/moxui-coder/skills/moxui-rust-patterns/SKILL.md (LOCAL-ONLY)

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_config_default() {
        let config = CacheConfig::default();
        assert_eq!(config.default_ttl, Duration::from_secs(5));
        assert_eq!(config.max_capacity, 10_000);
    }
}
