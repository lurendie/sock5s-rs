use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use chrono::{Days, Local, NaiveDate};

use crate::config::LogConfig;
use crate::error::Result;

static LOGGER: OnceLock<ProxyLogger> = OnceLock::new();

const MEGABYTE: u64 = 1024 * 1024;

pub struct ProxyLogger {
    dir: PathBuf,
    retention_days: u64,
    max_file_size: u64,
    state: Mutex<LoggerState>,
}

struct LoggerState {
    current_day: NaiveDate,
    current_index: u32,
    current_size: u64,
    file: File,
}

pub fn init(config: LogConfig) -> Result<()> {
    let logger = ProxyLogger::new(config)?;
    let _ = LOGGER.set(logger);
    Ok(())
}

pub fn log_access(protocol: &str, username: Option<&str>, client_addr: &str, client_ip: &str, target: &str) {
    if let Some(logger) = LOGGER.get() {
        let _ = logger.log_access(protocol, username, client_addr, client_ip, target);
    }
}

pub fn log_failure(
    event: &str,
    username: Option<&str>,
    client_addr: &str,
    client_ip: &str,
    target: Option<&str>,
    reason: &str,
) {
    if let Some(logger) = LOGGER.get() {
        let _ = logger.log_failure(event, username, client_addr, client_ip, target, reason);
    }
}

pub fn log_event(
    event: &str,
    username: Option<&str>,
    client_addr: &str,
    client_ip: &str,
    target: Option<&str>,
) {
    if let Some(logger) = LOGGER.get() {
        let _ = logger.log_event(event, username, client_addr, client_ip, target);
    }
}

impl ProxyLogger {
    fn new(config: LogConfig) -> Result<Self> {
        fs::create_dir_all(&config.dir)?;
        let today = Local::now().date_naive();
        let dir = config.dir.clone();
        let bootstrap = dir.join(".logger-bootstrap");
        let logger = Self {
            dir,
            retention_days: config.retention_days.max(1),
            max_file_size: config.max_file_size_mb.max(1) * MEGABYTE,
            state: Mutex::new(LoggerState {
                current_day: today,
                current_index: 0,
                current_size: 0,
                file: OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&bootstrap)?,
            }),
        };
        logger.cleanup(today)?;
        let (file, index, size) = Self::open_log_file(&logger.dir, today, logger.max_file_size)?;
        let mut state = logger.state.lock().unwrap();
        state.current_day = today;
        state.current_index = index;
        state.current_size = size;
        state.file = file;
        drop(state);
        let _ = fs::remove_file(bootstrap);
        Ok(logger)
    }

    fn log_access(
        &self,
        protocol: &str,
        username: Option<&str>,
        client_addr: &str,
        client_ip: &str,
        target: &str,
    ) -> Result<()> {
        let timestamp = Local::now();
        let line = format!(
            "{} protocol={} user={} client_ip={} client_addr={} target={}\n",
            timestamp.format("%Y-%m-%d %H:%M:%S"),
            protocol,
            username.unwrap_or("-"),
            client_ip,
            client_addr,
            target
        );

        let mut state = self.state.lock().unwrap();
        self.rotate_if_needed(&mut state, timestamp.date_naive(), line.len() as u64)?;
        state.file.write_all(line.as_bytes())?;
        state.file.flush()?;
        state.current_size += line.len() as u64;
        Ok(())
    }

    fn log_failure(
        &self,
        event: &str,
        username: Option<&str>,
        client_addr: &str,
        client_ip: &str,
        target: Option<&str>,
        reason: &str,
    ) -> Result<()> {
        let timestamp = Local::now();
        let line = format!(
            "{} event={} user={} client_ip={} client_addr={} target={} reason={}\n",
            timestamp.format("%Y-%m-%d %H:%M:%S"),
            event,
            username.unwrap_or("-"),
            client_ip,
            client_addr,
            target.unwrap_or("-"),
            reason
        );

        let mut state = self.state.lock().unwrap();
        self.rotate_if_needed(&mut state, timestamp.date_naive(), line.len() as u64)?;
        state.file.write_all(line.as_bytes())?;
        state.file.flush()?;
        state.current_size += line.len() as u64;
        Ok(())
    }

    fn log_event(
        &self,
        event: &str,
        username: Option<&str>,
        client_addr: &str,
        client_ip: &str,
        target: Option<&str>,
    ) -> Result<()> {
        let timestamp = Local::now();
        let line = format!(
            "{} event={} user={} client_ip={} client_addr={} target={}\n",
            timestamp.format("%Y-%m-%d %H:%M:%S"),
            event,
            username.unwrap_or("-"),
            client_ip,
            client_addr,
            target.unwrap_or("-"),
        );

        let mut state = self.state.lock().unwrap();
        self.rotate_if_needed(&mut state, timestamp.date_naive(), line.len() as u64)?;
        state.file.write_all(line.as_bytes())?;
        state.file.flush()?;
        state.current_size += line.len() as u64;
        Ok(())
    }

    fn rotate_if_needed(
        &self,
        state: &mut LoggerState,
        today: NaiveDate,
        incoming_size: u64,
    ) -> Result<()> {
        if state.current_day != today {
            self.cleanup(today)?;
            let (file, index, size) = Self::open_log_file(&self.dir, today, self.max_file_size)?;
            state.current_day = today;
            state.current_index = index;
            state.current_size = size;
            state.file = file;
            return Ok(());
        }

        if state.current_size > 0 && state.current_size + incoming_size > self.max_file_size {
            let next_index = state.current_index + 1;
            let path = Self::log_file_path(&self.dir, today, next_index);
            let file = OpenOptions::new().create(true).append(true).open(path)?;
            state.current_index = next_index;
            state.current_size = 0;
            state.file = file;
        }

        Ok(())
    }

    fn cleanup(&self, today: NaiveDate) -> Result<()> {
        let cutoff = today
            .checked_sub_days(Days::new(self.retention_days.saturating_sub(1)))
            .unwrap_or(today);

        for entry in fs::read_dir(&self.dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let Some(date) = Self::parse_date_from_path(&path) else {
                continue;
            };

            if date < cutoff {
                fs::remove_file(path)?;
            }
        }

        Ok(())
    }

    fn open_log_file(dir: &Path, day: NaiveDate, max_file_size: u64) -> Result<(File, u32, u64)> {
        let mut index = 0;

        loop {
            let path = Self::log_file_path(dir, day, index);
            let size = fs::metadata(&path).map(|x| x.len()).unwrap_or(0);

            if size < max_file_size || !path.exists() {
                let file = OpenOptions::new().create(true).append(true).open(path)?;
                return Ok((file, index, size));
            }

            index += 1;
        }
    }

    fn log_file_path(dir: &Path, day: NaiveDate, index: u32) -> PathBuf {
        let filename = if index == 0 {
            format!("proxy-{}.log", day.format("%Y-%m-%d"))
        } else {
            format!("proxy-{}.{}.log", day.format("%Y-%m-%d"), index)
        };
        dir.join(filename)
    }

    fn parse_date_from_path(path: &Path) -> Option<NaiveDate> {
        let stem = path.file_stem()?.to_str()?;
        let date_part = stem
            .strip_prefix("proxy-")?
            .split('.')
            .next()?;
        NaiveDate::parse_from_str(date_part, "%Y-%m-%d").ok()
    }
}
