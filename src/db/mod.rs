//! Database layer (rusqlite + r2d2 pool).
//!
//! See `docs/DATA_MODEL.md` for the schema and ER diagrams.

use crate::config::DatabaseConfig;

/// Create a database connection pool.
pub async fn create_pool(config: &DatabaseConfig) -> anyhow::Result<()> {
    // TODO: implement r2d2_sqlite pool with PRAGMAs
    // For now, return Ok so the app starts
    tracing::info!(path = %config.path, "Database pool (stub)");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_pool_stub() {
        let config = DatabaseConfig {
            path: ":memory:".to_string(),
            max_connections: 1,
            run_migrations: false,
        };
        assert!(create_pool(&config).await.is_ok());
    }
}
