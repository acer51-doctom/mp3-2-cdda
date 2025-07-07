use log::{LevelFilter};
use env_logger::Builder;
use std::io::Write;

pub fn initialize_logger() {
    Builder::new()
        .format(|buf, record| {
            writeln!(
                buf,
                "{} [{}] - {}: {}",
                chrono::Utc::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.target(),
                record.args()
            )
        })
        .filter(None, LevelFilter::Info) // Default to Info level; use RUST_LOG to override
        .init();
}

#[macro_export]
macro_rules! log_info {
    ($msg:expr) => {
        info!(target: "mp32cdda", $msg);
    };
    ($fmt:expr, $($arg:tt)*) => {
        info!(target: "mp32cdda", $fmt, $($arg)*);
    };
}

#[macro_export]
macro_rules! log_debug {
    ($msg:expr) => {
        debug!(target: "mp32cdda", $msg);
    };
    ($fmt:expr, $($arg:tt)*) => {
        debug!(target: "mp32cdda", $fmt, $($arg)*);
    };
}

#[macro_export]
macro_rules! log_warn {
    ($msg:expr) => {
        warn!(target: "mp32cdda", $msg);
    };
    ($fmt:expr, $($arg:tt)*) => {
        warn!(target: "mp32cdda", $fmt, $($arg)*);
    };
}

#[macro_export]
macro_rules! log_error {
    ($msg:expr) => {
        error!(target: "mp32cdda", $msg);
    };
    ($fmt:expr, $($arg:tt)*) => {
        error!(target: "mp32cdda", $fmt, $($arg)*);
    };
}