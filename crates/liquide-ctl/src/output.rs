use serde::Serialize;

use crate::cli::OutputFormat;

/// Unified output writer that respects the chosen output format.
pub struct Output {
    format: OutputFormat,
    color: bool,
    quiet: bool,
}

impl Output {
    pub fn new(format: OutputFormat, color: bool, quiet: bool) -> Self {
        Self {
            format,
            color,
            quiet,
        }
    }

    /// Print a value according to the configured format.
    pub fn print<T: Serialize + TextDisplay>(&self, value: &T) {
        match self.format {
            OutputFormat::Json => self.print_json(value),
            OutputFormat::Csv => self.print_csv(value),
            OutputFormat::Table => self.print_table(value),
            OutputFormat::Text => value.display_text(self.color),
        }
    }

    /// Print a simple message (only in text mode, respects quiet).
    pub fn message(&self, msg: &str) {
        if self.quiet {
            return;
        }
        match self.format {
            OutputFormat::Text | OutputFormat::Table => println!("{msg}"),
            OutputFormat::Json => {
                println!("{}", serde_json::json!({ "message": msg }));
            }
            OutputFormat::Csv => println!("{msg}"),
        }
    }

    /// Print a success message.
    pub fn success(&self, msg: &str) {
        if self.quiet {
            return;
        }
        if self.color {
            println!("\x1b[32m{msg}\x1b[0m");
        } else {
            println!("{msg}");
        }
    }

    /// Print a warning message.
    pub fn warn(&self, msg: &str) {
        if self.color {
            eprintln!("\x1b[33m⚠ {msg}\x1b[0m");
        } else {
            eprintln!("WARNING: {msg}");
        }
    }

    /// Print an error message.
    pub fn error(&self, msg: &str) {
        if self.color {
            eprintln!("\x1b[31m✗ {msg}\x1b[0m");
        } else {
            eprintln!("ERROR: {msg}");
        }
    }

    fn print_json<T: Serialize>(&self, value: &T) {
        match serde_json::to_string_pretty(value) {
            Ok(json) => println!("{json}"),
            Err(e) => eprintln!("Failed to serialize to JSON: {e}"),
        }
    }

    fn print_csv<T: Serialize>(&self, _value: &T) {
        // CSV output will be implemented per-type via TextDisplay
        // For now, fall back to JSON
        self.print_json(_value);
    }

    fn print_table<T: Serialize + TextDisplay>(&self, value: &T) {
        // Table output via the TextDisplay trait
        value.display_text(self.color);
    }

    /// Whether output is in quiet mode.
    pub fn is_quiet(&self) -> bool {
        self.quiet
    }

    /// Whether color is enabled.
    pub fn is_color(&self) -> bool {
        self.color
    }
}

/// Trait for types that can render themselves as human-readable text.
pub trait TextDisplay {
    fn display_text(&self, color: bool);
}

/// Determine whether color should be enabled based on the --color flag.
pub fn should_colorize(color: &crate::cli::ColorWhen) -> bool {
    match color {
        crate::cli::ColorWhen::Always => true,
        crate::cli::ColorWhen::Never => false,
        crate::cli::ColorWhen::Auto => {
            // Auto: color if stdout is a terminal
            atty_check()
        }
    }
}

fn atty_check() -> bool {
    // Simple heuristic: check if NO_COLOR env var is set
    if std::env::var("NO_COLOR").is_ok() {
        return false;
    }
    // On Unix we could check isatty; for now default to true
    true
}
