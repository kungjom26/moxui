//! Shared application state.
//!
//! Holds all dependencies that handlers need via `axum::extract::State<AppState>`.

use std::sync::Arc;

use crate::config::Config;

/// Shared application state (cloned for each handler).
#[derive(Clone)]
pub struct AppState {
    /// Application configuration.
    pub config: Arc<Config>,
}

impl AppState {
    /// Create new state from config.
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(config),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn test_state_new() {
        let config = Config::load().unwrap();
        let state = AppState::new(config);
        assert_eq!(state.config.server.bind, "0.0.0.0:8080");
    }
}
