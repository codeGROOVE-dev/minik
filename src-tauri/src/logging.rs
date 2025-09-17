use anyhow::Result;
use log::LevelFilter;
use log4rs::{
    append::{
        console::ConsoleAppender,
        rolling_file::policy::compound::{
            roll::fixed_window::FixedWindowRoller, trigger::size::SizeTrigger, CompoundPolicy,
        },
        rolling_file::RollingFileAppender,
    },
    config::{Appender, Config, Root},
    encode::pattern::PatternEncoder,
};
use std::path::PathBuf;

/// Initialize the logging system with both console and file outputs
pub fn init_logging() -> Result<()> {
    // Determine the log directory based on the platform
    let log_dir = get_log_directory()?;
    std::fs::create_dir_all(&log_dir)?;

    let log_file = log_dir.join("minik.log");
    let archive_pattern = log_dir.join("archive").join("minik.{}.log");

    // Create archive directory if it doesn't exist
    if let Some(archive_dir) = archive_pattern.parent() {
        std::fs::create_dir_all(archive_dir)?;
    }

    // Pattern for log messages
    let pattern = "{d(%Y-%m-%d %H:%M:%S%.3f)} [{l}] [{T}] {m}\n";

    // Console appender
    let console = ConsoleAppender::builder()
        .encoder(Box::new(PatternEncoder::new(pattern)))
        .build();

    // Rolling file appender with size-based rotation
    // Rotate when file reaches 10MB, keep 5 archived files
    let file_appender = RollingFileAppender::builder()
        .encoder(Box::new(PatternEncoder::new(pattern)))
        .build(
            log_file,
            Box::new(CompoundPolicy::new(
                Box::new(SizeTrigger::new(10 * 1024 * 1024)), // 10MB
                Box::new(
                    FixedWindowRoller::builder().base(0).build(
                        archive_pattern
                            .to_str()
                            .ok_or_else(|| anyhow::anyhow!("Invalid archive pattern"))?,
                        5,
                    )?,
                ),
            )),
        )?;

    // Build the configuration
    let config = Config::builder()
        .appender(Appender::builder().build("console", Box::new(console)))
        .appender(Appender::builder().build("file", Box::new(file_appender)))
        .build(
            Root::builder()
                .appender("console")
                .appender("file")
                .build(LevelFilter::Info),
        )?;

    // Initialize log4rs
    log4rs::init_config(config)?;

    log::info!("===========================================");
    log::info!("Minik application started");
    log::info!("Log directory: {}", log_dir.display());
    log::info!("Platform: {}", std::env::consts::OS);
    log::info!("Architecture: {}", std::env::consts::ARCH);
    log::info!("===========================================");

    Ok(())
}

/// Get the appropriate log directory for the current platform
fn get_log_directory() -> Result<PathBuf> {
    let log_dir = if cfg!(target_os = "macos") {
        // macOS: ~/Library/Logs/Minik/
        dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?
            .join("Library")
            .join("Logs")
            .join("Minik")
    } else if cfg!(target_os = "windows") {
        // Windows: %LOCALAPPDATA%\Minik\logs\
        dirs::data_local_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find local data directory"))?
            .join("Minik")
            .join("logs")
    } else {
        // Linux and others: ~/.local/share/minik/logs/
        dirs::data_local_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find local data directory"))?
            .join("minik")
            .join("logs")
    };

    Ok(log_dir)
}

/// Macro to log function entry with arguments
#[macro_export]
macro_rules! log_entry {
    ($func_name:expr) => {
        log::debug!("Entering function: {}", $func_name);
    };
    ($func_name:expr, $($arg:tt)*) => {
        log::debug!("Entering function: {} with args: {}", $func_name, format!($($arg)*));
    };
}

/// Macro to log function exit with result
#[macro_export]
macro_rules! log_exit {
    ($func_name:expr, $result:expr) => {
        match &$result {
            Ok(val) => log::debug!("Exiting function: {} with success: {:?}", $func_name, val),
            Err(err) => log::error!("Exiting function: {} with error: {:?}", $func_name, err),
        }
    };
}

/// Macro to log errors with context
#[macro_export]
macro_rules! log_error_context {
    ($context:expr, $error:expr) => {
        log::error!("[{}] Error: {:?}", $context, $error);
    };
}
