use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;

use crate::error::Result;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub listen: SocketAddr,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub access: AccessConfig,
    #[serde(default)]
    pub log: LogConfig,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct AuthConfig {
    #[serde(default)]
    pub users: Vec<UserEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct UserEntry {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LogConfig {
    #[serde(default = "default_log_dir")]
    pub dir: PathBuf,
    #[serde(default = "default_retention_days")]
    pub retention_days: u64,
    #[serde(default = "default_max_file_size_mb")]
    pub max_file_size_mb: u64,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct AccessConfig {
    #[serde(default)]
    pub mode: AccessMode,
    #[serde(default)]
    pub allow_domains: bool,
    #[serde(default)]
    pub ips: Vec<IpAddr>,
    #[serde(default)]
    pub cidrs: Vec<IpNet>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AccessMode {
    #[default]
    Blacklist,
    Whitelist,
}

#[derive(Debug, Clone)]
pub struct AuthState {
    users: HashSet<UserEntry>,
}

#[derive(Debug, Clone)]
pub struct AccessState {
    mode: AccessMode,
    allow_domains: bool,
    ips: HashSet<IpAddr>,
    cidrs: Vec<IpNet>,
}

fn default_log_dir() -> PathBuf {
    PathBuf::from("logs")
}

fn default_retention_days() -> u64 {
    7
}

fn default_max_file_size_mb() -> u64 {
    100
}

impl AppConfig {
    pub fn from_file(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn save_to_file(&self, path: &str) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

impl AuthConfig {
    pub fn to_state(&self) -> Option<AuthState> {
        if self.users.is_empty() {
            return None;
        }
        Some(AuthState {
            users: self.users.iter().cloned().collect(),
        })
    }
}

impl AuthState {
    pub fn verify(&self, username: &str, password: &str) -> bool {
        self.users.contains(&UserEntry {
            username: username.to_owned(),
            password: password.to_owned(),
        })
    }
}

impl AccessConfig {
    pub fn to_state(&self) -> Option<AccessState> {
        if self.ips.is_empty()
            && self.cidrs.is_empty()
            && !self.allow_domains
            && self.mode == AccessMode::Blacklist
        {
            return None;
        }

        Some(AccessState {
            mode: self.mode,
            allow_domains: self.allow_domains,
            ips: self.ips.iter().copied().collect(),
            cidrs: self.cidrs.clone(),
        })
    }
}

impl AccessState {
    pub fn allow_domains(&self) -> bool {
        self.allow_domains
    }

    pub fn is_denied(&self, ip: IpAddr) -> bool {
        let matched = self.ips.contains(&ip) || self.cidrs.iter().any(|net| net.contains(&ip));
        match self.mode {
            AccessMode::Blacklist => matched,
            AccessMode::Whitelist => !matched,
        }
    }
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            dir: default_log_dir(),
            retention_days: default_retention_days(),
            max_file_size_mb: default_max_file_size_mb(),
        }
    }
}
