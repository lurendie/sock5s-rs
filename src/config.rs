use ipnet::IpNet;
use serde::Deserialize;
use std::collections::HashSet;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;

use crate::error::Result;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub listen: SocketAddr,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub access: AccessConfig,
    #[serde(default)]
    pub log: LogConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AuthConfig {
    #[serde(default)]
    pub users: Vec<UserEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
pub struct UserEntry {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LogConfig {
    #[serde(default = "default_log_dir")]
    pub dir: PathBuf,
    #[serde(default = "default_retention_days")]
    pub retention_days: u64,
    #[serde(default = "default_max_file_size_mb")]
    pub max_file_size_mb: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AccessConfig {
    #[serde(default)]
    pub mode: AccessMode,
    #[serde(default)]
    pub ips: Vec<IpAddr>,
    #[serde(default)]
    pub cidrs: Vec<IpNet>,
    #[serde(default)]
    pub deny_ips: Vec<IpAddr>,
    #[serde(default)]
    pub deny_cidrs: Vec<IpNet>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
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
        let mut ips = self.ips.clone();
        let mut cidrs = self.cidrs.clone();

        if ips.is_empty() && cidrs.is_empty() {
            ips = self.deny_ips.clone();
            cidrs = self.deny_cidrs.clone();
        }

        if ips.is_empty() && cidrs.is_empty() {
            return None;
        }

        Some(AccessState {
            mode: self.mode,
            ips: ips.iter().copied().collect(),
            cidrs,
        })
    }
}

impl AccessState {
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
