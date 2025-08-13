use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub scanner: ScannerConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannerConfig {
    pub auto_scan_paths: Vec<PathBuf>,
    pub scan_interval_minutes: Option<u64>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 3000,
            },
            database: DatabaseConfig {
                url: "sqlite://medianator.db".to_string(),
            },
            scanner: ScannerConfig {
                auto_scan_paths: vec![],
                scan_interval_minutes: None,
            },
        }
    }
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let mut config = Self::default();

        if let Ok(host) = std::env::var("SERVER_HOST") {
            config.server.host = host;
        }

        if let Ok(port) = std::env::var("SERVER_PORT") {
            config.server.port = port.parse()?;
        }

        if let Ok(db_url) = std::env::var("DATABASE_URL") {
            config.database.url = db_url;
        }

        if let Ok(scan_paths) = std::env::var("AUTO_SCAN_PATHS") {
            config.scanner.auto_scan_paths = scan_paths
                .split(',')
                .map(|p| PathBuf::from(p.trim()))
                .collect();
        }

        if let Ok(interval) = std::env::var("SCAN_INTERVAL_MINUTES") {
            config.scanner.scan_interval_minutes = Some(interval.parse()?);
        }

        Ok(config)
    }
}