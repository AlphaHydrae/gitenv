use log::Level;
use std::fmt::Display;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

const ANSI_RESET: &str = "\x1b[0m";
const ANSI_RED: &str = "\x1b[31m";
const ANSI_YELLOW: &str = "\x1b[33m";
const ANSI_GREEN: &str = "\x1b[32m";
const ANSI_BLUE: &str = "\x1b[34m";
const ANSI_MAGENTA: &str = "\x1b[35m";

static LOGGER_USE_COLOR: AtomicBool = AtomicBool::new(false);

// ── Log subscriber ────────────────────────────────────────────────────────────
//
// The global logger is required by the `log` crate API. Color behavior is
// configured explicitly from the composition root via `init`.

struct SimpleLogger;

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            eprintln!(
                "{}",
                format_record(record, unix_timestamp_millis(), configured_use_color())
            );
        }
    }

    fn flush(&self) {}
}

static LOGGER: SimpleLogger = SimpleLogger;

fn encode_color_mode(use_color: bool) -> bool {
    use_color
}

fn configured_use_color() -> bool {
    LOGGER_USE_COLOR.load(Ordering::Relaxed)
}

fn timestamp_millis_from(now: SystemTime, epoch: SystemTime) -> u128 {
    match now.duration_since(epoch) {
        Ok(duration) => duration.as_millis(),
        Err(_) => 0,
    }
}

fn unix_timestamp_millis() -> u128 {
    timestamp_millis_from(SystemTime::now(), UNIX_EPOCH)
}

fn format_unix_millis_iso8601(unix_millis: u128) -> String {
    let bounded_millis = unix_millis.min(i64::MAX as u128) as i64;
    let unix_nanos = i128::from(bounded_millis) * 1_000_000;

    let timestamp = match OffsetDateTime::from_unix_timestamp_nanos(unix_nanos) {
        Ok(value) => value,
        Err(_) => OffsetDateTime::UNIX_EPOCH,
    };

    match timestamp.format(&Rfc3339) {
        Ok(text) => text,
        Err(_) => "1970-01-01T00:00:00Z".to_string(),
    }
}

fn level_color(level: Level) -> &'static str {
    match level {
        Level::Error => ANSI_RED,
        Level::Warn => ANSI_YELLOW,
        Level::Info => ANSI_GREEN,
        Level::Debug => ANSI_BLUE,
        Level::Trace => ANSI_MAGENTA,
    }
}

fn format_record(record: &log::Record, timestamp_millis: u128, use_color: bool) -> String {
    let timestamp_iso8601 = format_unix_millis_iso8601(timestamp_millis);
    let level_text = record.level().to_string();
    let rendered_level = if use_color {
        format!(
            "{}{}{}",
            level_color(record.level()),
            level_text,
            ANSI_RESET
        )
    } else {
        level_text
    };

    format!(
        "[{timestamp_iso8601}] [{rendered_level}] {}: {}",
        record.target(),
        record.args()
    )
}

/// Initialize the global logger with the given level filter and color decision.
///
/// Safe to call more than once — subsequent calls update level and color mode.
/// The `use_color_for_stderr` boolean should be precomputed at startup based on
/// ColorMode and terminal detection to avoid repeated terminal probes.
pub fn init(level: log::LevelFilter, use_color_for_stderr: bool) {
    LOGGER_USE_COLOR.store(encode_color_mode(use_color_for_stderr), Ordering::Relaxed);
    // Ignore the error: it means the logger was already installed, which is
    // fine (e.g. when running multiple tests in the same process).
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(level);
}

// ── Domain-module helpers ─────────────────────────────────────────────────────

fn emit(target: &'static str, level: Level, event: &str, details: impl Display) {
    log::log!(target: target, level, "event={event} {details}");
}

pub(crate) fn config(level: Level, event: &str, details: impl Display) {
    emit("gitenv::config", level, event, details);
}

pub(crate) fn intent(level: Level, event: &str, details: impl Display) {
    emit("gitenv::intent", level, event, details);
}

pub(crate) fn operation(level: Level, event: &str, details: impl Display) {
    emit("gitenv::operation", level, event, details);
}

pub(crate) fn actions(level: Level, event: &str, details: impl Display) {
    emit("gitenv::actions", level, event, details);
}

pub(crate) fn status(level: Level, event: &str, details: impl Display) {
    emit("gitenv::status", level, event, details);
}

pub(crate) fn system(level: Level, event: &str, details: impl Display) {
    emit("gitenv::system", level, event, details);
}

#[cfg(test)]
mod tests {
    use super::{
        ANSI_BLUE, ANSI_RESET, LOGGER, format_record, format_unix_millis_iso8601,
        timestamp_millis_from,
    };
    use log::{Level, Record};
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn show_timestamped_plain_log_lines_when_color_is_disabled() {
        let record = Record::builder()
            .level(Level::Info)
            .target("gitenv::status")
            .args(format_args!("event=inspect path=/tmp/example"))
            .build();

        let rendered = format_record(&record, 1_717_500_000_123, false);

        assert_eq!(
            rendered,
            "[2024-06-04T11:20:00.123Z] [INFO] gitenv::status: event=inspect path=/tmp/example"
        );
    }

    #[test]
    fn show_timestamped_colored_log_lines_when_color_is_enabled() {
        let record = Record::builder()
            .level(Level::Debug)
            .target("gitenv::operation")
            .args(format_args!("event=derive operations=1"))
            .build();

        let rendered = format_record(&record, 42, true);

        assert_eq!(
            rendered,
            format!(
                "[1970-01-01T00:00:00.042Z] [{}DEBUG{}] gitenv::operation: event=derive operations=1",
                ANSI_BLUE, ANSI_RESET
            )
        );
    }

    #[test]
    fn return_zero_for_timestamps_before_the_epoch() {
        let before_epoch = UNIX_EPOCH
            .checked_sub(Duration::from_secs(1))
            .expect("a one-second subtraction from UNIX_EPOCH should be representable");

        let timestamp = timestamp_millis_from(before_epoch, UNIX_EPOCH);

        assert_eq!(timestamp, 0);
    }

    #[test]
    fn format_unix_millis_as_iso8601_utc() {
        assert_eq!(
            format_unix_millis_iso8601(1_717_500_000_123),
            "2024-06-04T11:20:00.123Z"
        );
    }

    #[test]
    fn fall_back_to_epoch_when_timestamp_is_out_of_range() {
        assert_eq!(
            format_unix_millis_iso8601(u128::MAX),
            "1970-01-01T00:00:00Z"
        );
    }

    #[test]
    fn show_colorized_level_for_all_supported_levels() {
        let expectations = [
            (Level::Error, "\x1b[31mERROR\x1b[0m"),
            (Level::Warn, "\x1b[33mWARN\x1b[0m"),
            (Level::Info, "\x1b[32mINFO\x1b[0m"),
            (Level::Debug, "\x1b[34mDEBUG\x1b[0m"),
            (Level::Trace, "\x1b[35mTRACE\x1b[0m"),
        ];

        for (level, colored_level) in expectations {
            let record = Record::builder()
                .level(level)
                .target("gitenv::test")
                .args(format_args!("event=test"))
                .build();

            let rendered = format_record(&record, 7, true);
            assert_eq!(
                rendered,
                format!("[1970-01-01T00:00:00.007Z] [{colored_level}] gitenv::test: event=test")
            );
        }
    }

    #[test]
    fn flush_the_logger_without_panicking() {
        use log::Log;

        LOGGER.flush();
    }
}
