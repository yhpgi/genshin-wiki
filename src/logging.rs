use colored::*;
use once_cell::sync::Lazy;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogLevel {
    Step,
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ColorCode {
    Green,
    Yellow,
    Red,
    Blue,
    Purple,
    White,
}

static COLOR_MAP: Lazy<HashMap<ColorCode, &'static str>> = Lazy::new(|| {
    HashMap::from([
        (ColorCode::Green, "green"),
        (ColorCode::Yellow, "yellow"),
        (ColorCode::Red, "red"),
        (ColorCode::Blue, "cyan"),
        (ColorCode::Purple, "magenta"),
        (ColorCode::White, "white"),
    ])
});

static LOG_LEVEL_CONFIG: Lazy<HashMap<LogLevel, (String, ColorCode)>> = Lazy::new(|| {
    HashMap::from([
        (LogLevel::Step, ("STEP".to_string(), ColorCode::Purple)),
        (LogLevel::Info, ("INFO".to_string(), ColorCode::Blue)),
        (LogLevel::Success, ("SUCCESS".to_string(), ColorCode::Green)),
        (
            LogLevel::Warning,
            ("WARNING".to_string(), ColorCode::Yellow),
        ),
        (LogLevel::Error, ("ERROR".to_string(), ColorCode::Red)),
    ])
});

static MAX_LEVEL_LEN: Lazy<usize> = Lazy::new(|| {
    LOG_LEVEL_CONFIG
        .values()
        .map(|(s, _)| s.len())
        .max()
        .unwrap_or(7)
});

static MAX_BRACKET_VISUAL_WIDTH: Lazy<usize> = Lazy::new(|| *MAX_LEVEL_LEN + 4);
const MIN_PADDING_AFTER_BRACKET: usize = 1;

static TARGET_TOTAL_PREFIX_WIDTH: Lazy<usize> =
    Lazy::new(|| *MAX_BRACKET_VISUAL_WIDTH + MIN_PADDING_AFTER_BRACKET);

static LOG_PREFIXES: Lazy<HashMap<LogLevel, String>> = Lazy::new(|| {
    colored::control::set_override(true);

    LOG_LEVEL_CONFIG
        .iter()
        .map(|(level, (level_str, color_code))| {
            let color_name = COLOR_MAP.get(color_code).unwrap_or(&"white");
            let level_part_inside = format!(" {} ", level_str);

            let current_visual_width = level_str.len() + 4;
            let padding_count = TARGET_TOTAL_PREFIX_WIDTH.saturating_sub(current_visual_width);
            let level_part_colored = level_part_inside.color(*color_name).bold();
            let level_part_bracketed = format!("[{}]", level_part_colored);
            (
                *level,
                format!("{}{}", level_part_bracketed, " ".repeat(padding_count)),
            )
        })
        .collect()
});

pub fn setup_logging() {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }

    let format = tracing_subscriber::fmt::format()
        .without_time()
        .with_level(false)
        .with_target(false)
        .compact();

    tracing_subscriber::fmt()
        .event_format(format)
        .with_ansi(true)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
}

pub fn log(level: LogLevel, message: &str) {
    let prefix = LOG_PREFIXES
        .get(&level)
        .cloned()
        .unwrap_or_else(|| format!("[{:<7}] ", format!("{:?}", level)));

    match level {
        LogLevel::Step => tracing::info!(target: "step", "{}", format!("{}{}", prefix, message)),
        LogLevel::Info => tracing::info!("{}", format!("{}{}", prefix, message)),
        LogLevel::Success => tracing::info!("{}", format!("{}{}", prefix, message)),
        LogLevel::Warning => tracing::warn!("{}", format!("{}{}", prefix, message)),
        LogLevel::Error => tracing::error!("{}", format!("{}{}", prefix, message)),
    }
}
