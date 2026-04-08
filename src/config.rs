use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Top-level configuration for datap4k-mcp.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    pub sources: Sources,
    pub index: IndexConfig,
    pub server: ServerConfig,
}

impl Config {
    /// Return the default config file path: ~/.datap4k-mcp/config.toml
    pub fn default_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".datap4k-mcp")
            .join("config.toml")
    }

    /// Load config from a specific path. Returns default config if the file does not exist.
    pub fn load_from(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&contents)?;
        Ok(config)
    }

    /// Load config from the default path (~/.datap4k-mcp/config.toml).
    pub fn load() -> Result<Self> {
        Self::load_from(&Self::default_path())
    }

    /// Save config to a specific path. Creates parent directories if needed.
    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self)?;
        std::fs::write(path, contents)?;
        Ok(())
    }

    /// Save config to the default path (~/.datap4k-mcp/config.toml).
    pub fn save(&self) -> Result<()> {
        self.save_to(&Self::default_path())
    }

    /// Add or update a version source. If a source with the same version code already exists,
    /// its path and parser are updated in-place.
    pub fn add_version(&mut self, path: &str, version: &str, parser: &str) {
        if let Some(existing) = self
            .sources
            .versions
            .iter_mut()
            .find(|v| v.version == version)
        {
            existing.path = path.to_string();
            existing.parser = parser.to_string();
        } else {
            self.sources.versions.push(VersionSource {
                path: path.to_string(),
                version: version.to_string(),
                parser: parser.to_string(),
            });
        }
    }
}

/// Collection of p4k version sources to index.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Sources {
    pub versions: Vec<VersionSource>,
}

/// A single extracted p4k data directory with its version and parser hint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionSource {
    /// Absolute path to the extracted p4k data directory.
    pub path: String,
    /// Stable version code, e.g. "4.7.0-live".
    pub version: String,
    /// Parser to use: "auto" (default), "scdatatools", etc.
    #[serde(default = "default_parser")]
    pub parser: String,
}

fn default_parser() -> String {
    "auto".to_string()
}

/// Configuration for the graph index storage location.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexConfig {
    /// Path to the Cozo/SQLite index directory.
    pub path: String,
}

impl Default for IndexConfig {
    fn default() -> Self {
        let path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".datap4k-mcp")
            .join("index")
            .to_string_lossy()
            .to_string();
        Self { path }
    }
}

/// MCP server runtime configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Log level: "trace", "debug", "info" (default), "warn", "error"
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            log_level: default_log_level(),
        }
    }
}

fn default_log_level() -> String {
    "info".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.sources.versions.is_empty());
        assert_eq!(config.server.log_level, "info");
    }

    #[test]
    fn test_config_roundtrip() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.toml");

        let mut config = Config::default();
        config.sources.versions.push(VersionSource {
            path: "/data/4.7.0-live".to_string(),
            version: "4.7.0-live".to_string(),
            parser: "auto".to_string(),
        });

        config.save_to(&config_path).unwrap();
        let loaded = Config::load_from(&config_path).unwrap();

        assert_eq!(loaded.sources.versions.len(), 1);
        assert_eq!(loaded.sources.versions[0].version, "4.7.0-live");
    }

    #[test]
    fn test_config_missing_file_returns_default() {
        let config = Config::load_from(std::path::Path::new("/nonexistent/config.toml")).unwrap();
        assert!(config.sources.versions.is_empty());
    }

    #[test]
    fn test_add_version() {
        let mut config = Config::default();
        config.add_version("/data/4.7.0", "4.7.0-live", "auto");
        assert_eq!(config.sources.versions.len(), 1);

        config.add_version("/data/4.7.0-new", "4.7.0-live", "scdatatools");
        assert_eq!(config.sources.versions.len(), 1);
        assert_eq!(config.sources.versions[0].path, "/data/4.7.0-new");
    }
}
