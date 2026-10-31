//! Log lines are "<date> <time> [LEVEL] message" in local time, the format the LuCI page shows.

use std::{
    ffi::CStr,
    fs::{File, OpenOptions},
    io::{self, Write},
    os::unix::fs::{FileExt, OpenOptionsExt},
    sync::{Mutex, PoisonError},
};

use log::{Level, LevelFilter, Log, Metadata, Record};

const MAX_LOG_SIZE: u64 = 128 << 10;

struct Logger(Mutex<File>);

pub fn init(path: &str) -> io::Result<()> {
    let file = OpenOptions::new()
        .read(true)
        .append(true)
        .create(true)
        .mode(0o600)
        .open(path)?;
    log::set_logger(Box::leak(Box::new(Logger(Mutex::new(file))))).expect("the logger is set once");
    // Loads the time zone while descriptors are free: musl keeps the first result, UTC if no file could be opened.
    now(c"", false);
    log::set_max_level(LevelFilter::Warn);
    Ok(())
}

pub fn parse_level(name: &str) -> Option<LevelFilter> {
    match name {
        "warning" => Some(LevelFilter::Warn),
        "info" => Some(LevelFilter::Info),
        _ => None,
    }
}

/// The warnings and errors of shadowsocks-rust are about single packets or connections, which mio logs as
/// info; its info is only the UDP address, which the startup line of mio gives.
/// (The mio crate under tokio shares the target but only logs at trace, which is compiled out.)
fn level(metadata: &Metadata) -> Option<Level> {
    let target = metadata.target();
    if target == "mio" || target.starts_with("mio::") {
        Some(metadata.level())
    } else {
        (metadata.level() < Level::Info).then_some(Level::Info)
    }
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        level(metadata).is_some_and(|level| level <= log::max_level())
    }

    fn log(&self, record: &Record) {
        let Some(level) = level(record.metadata()).filter(|level| *level <= log::max_level()) else {
            return;
        };
        let name = match level {
            Level::Error => "ERROR",
            Level::Warn => "WARNING",
            _ => "INFO",
        };
        let line = format!("{} [{name}] {}\n", now(c"%Y-%m-%d %H:%M:%S", false), record.args());
        let file = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        trim(&file);
        _ = (&*file).write_all(line.as_bytes());
    }

    fn flush(&self) {}
}

/// Once the file grows past MAX_LOG_SIZE, keeps only the newest half.
fn trim(mut file: &File) {
    let Ok(size) = file.metadata().map(|metadata| metadata.len()) else {
        return;
    };
    if size <= MAX_LOG_SIZE {
        return;
    }
    let mut tail = vec![0; (MAX_LOG_SIZE / 2) as usize];
    let n = file.read_at(&mut tail, size - MAX_LOG_SIZE / 2).unwrap_or(0);
    // Drop the partial first line.
    let start = tail[..n].iter().position(|&b| b == b'\n').map_or(n, |i| i + 1);
    if file.set_len(0).is_ok() {
        _ = file.write_all(&tail[start..n]);
    }
}

/// Formats the current time with strftime(3), in local time or in UTC.
pub fn now(format: &CStr, utc: bool) -> String {
    let mut buffer = [0u8; 64];
    // SAFETY: tm is filled in before use and strftime writes at most buffer.len() bytes.
    let n = unsafe {
        let time = libc::time(std::ptr::null_mut());
        let mut tm = std::mem::zeroed();
        if utc {
            libc::gmtime_r(&time, &mut tm);
        } else {
            libc::localtime_r(&time, &mut tm);
        }
        libc::strftime(buffer.as_mut_ptr().cast(), buffer.len(), format.as_ptr(), &tm)
    };
    String::from_utf8_lossy(&buffer[..n]).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trim_keeps_whole_lines() {
        let path = std::env::temp_dir().join(format!("mio-log-{}", std::process::id()));
        let mut file = OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .open(&path)
            .unwrap();
        for i in 0..10000 {
            writeln!(file, "line {i:05} of the log").unwrap();
        }
        trim(&file);
        let data = std::fs::read_to_string(&path).unwrap();
        _ = std::fs::remove_file(&path);
        assert!(data.len() as u64 <= MAX_LOG_SIZE / 2);
        assert!(data.starts_with("line ") && data.ends_with("line 09999 of the log\n"));
    }

    #[test]
    fn upstream_messages_are_info() {
        let upstream = Metadata::builder()
            .target("shadowsocks_service::server")
            .level(Level::Error)
            .build();
        assert_eq!(level(&upstream), Some(Level::Info));
        let upstream = Metadata::builder()
            .target("shadowsocks_service::server")
            .level(Level::Info)
            .build();
        assert_eq!(level(&upstream), None);
        let own = Metadata::builder().target("mio::tcp").level(Level::Warn).build();
        assert_eq!(level(&own), Some(Level::Warn));
    }
}
