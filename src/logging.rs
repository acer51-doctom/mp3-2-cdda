use log::LevelFilter;
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
        .filter(None, LevelFilter::Info) // Default to Info; override with RUST_LOG
        .init();
}

#[macro_export]
macro_rules! log_info {
    ($msg:expr) => {
        log::info!(target: "mp32cdda", $msg);
    };
    ($fmt:expr, $($arg:tt)*) => {
        log::info!(target: "mp32cdda", $fmt, $($arg)*);
    };
}

#[macro_export]
macro_rules! log_debug {
    ($msg:expr) => {
        log::debug!(target: "mp32cdda", $msg);
    };
    ($fmt:expr, $($arg:tt)*) => {
        log::debug!(target: "mp32cdda", $fmt, $($arg)*);
    };
}

#[macro_export]
macro_rules! log_warn {
    ($msg:expr) => {
        log::warn!(target: "mp32cdda", $msg);
    };
    ($fmt:expr, $($arg:tt)*) => {
        log::warn!(target: "mp32cdda", $fmt, $($arg)*);
    };
}

#[macro_export]
macro_rules! log_error {
    ($msg:expr) => {
        log::error!(target: "mp32cdda", $msg);
    };
    ($fmt:expr, $($arg:tt)*) => {
        log::error!(target: "mp32cdda", $fmt, $($arg)*);
    };
}