use crate::locale::Locale;

/// Number formatting rules for a specific locale.
#[derive(Debug, Clone)]
struct NumberFormatRules {
    /// Decimal separator (e.g., "." or ",").
    decimal_sep: &'static str,
    /// Thousands grouping separator (e.g., "," or ".").
    thousands_sep: &'static str,
    /// Grouping size (typically 3).
    grouping: u8,
    /// Currency symbol.
    currency_symbol: &'static str,
    /// If true, currency symbol comes before the number.
    currency_prefix: bool,
    /// Space between currency symbol and number.
    currency_space: bool,
    /// Percent format: true = "50 %" (space before %), false = "50%"
    percent_space: bool,
}

fn rules_for_locale(locale: &Locale) -> NumberFormatRules {
    let key = match locale.territory.as_deref() {
        Some(terr) => (locale.language.as_str(), terr),
        None => (locale.language.as_str(), ""),
    };

    match key {
        ("en", "US") | ("en", "") => NumberFormatRules {
            decimal_sep: ".",
            thousands_sep: ",",
            grouping: 3,
            currency_symbol: "$",
            currency_prefix: true,
            currency_space: false,
            percent_space: false,
        },
        ("en", "GB") => NumberFormatRules {
            decimal_sep: ".",
            thousands_sep: ",",
            grouping: 3,
            currency_symbol: "\u{00a3}",
            currency_prefix: true,
            currency_space: false,
            percent_space: false,
        },
        ("de", _) => NumberFormatRules {
            decimal_sep: ",",
            thousands_sep: ".",
            grouping: 3,
            currency_symbol: "\u{20ac}",
            currency_prefix: false,
            currency_space: true,
            percent_space: true,
        },
        ("fr", _) => NumberFormatRules {
            decimal_sep: ",",
            thousands_sep: "\u{202f}",  // narrow no-break space
            grouping: 3,
            currency_symbol: "\u{20ac}",
            currency_prefix: false,
            currency_space: true,
            percent_space: true,
        },
        ("es", _) => NumberFormatRules {
            decimal_sep: ",",
            thousands_sep: ".",
            grouping: 3,
            currency_symbol: "\u{20ac}",
            currency_prefix: false,
            currency_space: true,
            percent_space: true,
        },
        ("pt", "BR") => NumberFormatRules {
            decimal_sep: ",",
            thousands_sep: ".",
            grouping: 3,
            currency_symbol: "R$",
            currency_prefix: true,
            currency_space: true,
            percent_space: false,
        },
        ("pt", _) => NumberFormatRules {
            decimal_sep: ",",
            thousands_sep: ".",
            grouping: 3,
            currency_symbol: "\u{20ac}",
            currency_prefix: false,
            currency_space: true,
            percent_space: true,
        },
        ("ja", _) => NumberFormatRules {
            decimal_sep: ".",
            thousands_sep: ",",
            grouping: 3,
            currency_symbol: "\u{00a5}",
            currency_prefix: true,
            currency_space: false,
            percent_space: false,
        },
        ("zh", _) => NumberFormatRules {
            decimal_sep: ".",
            thousands_sep: ",",
            grouping: 3,
            currency_symbol: "\u{00a5}",
            currency_prefix: true,
            currency_space: false,
            percent_space: false,
        },
        ("ko", _) => NumberFormatRules {
            decimal_sep: ".",
            thousands_sep: ",",
            grouping: 3,
            currency_symbol: "\u{20a9}",
            currency_prefix: true,
            currency_space: false,
            percent_space: false,
        },
        ("ar", _) => NumberFormatRules {
            decimal_sep: "\u{066b}",  // Arabic decimal separator
            thousands_sep: "\u{066c}",  // Arabic thousands separator
            grouping: 3,
            currency_symbol: "ر.س",
            currency_prefix: false,
            currency_space: true,
            percent_space: false,
        },
        ("ru", _) => NumberFormatRules {
            decimal_sep: ",",
            thousands_sep: "\u{00a0}",  // no-break space
            grouping: 3,
            currency_symbol: "\u{20bd}",
            currency_prefix: false,
            currency_space: true,
            percent_space: true,
        },
        ("hi", _) => NumberFormatRules {
            decimal_sep: ".",
            thousands_sep: ",",
            grouping: 3,  // simplified; Hindi uses 3 for last group, 2 for rest
            currency_symbol: "\u{20b9}",
            currency_prefix: true,
            currency_space: false,
            percent_space: false,
        },
        ("it", _) => NumberFormatRules {
            decimal_sep: ",",
            thousands_sep: ".",
            grouping: 3,
            currency_symbol: "\u{20ac}",
            currency_prefix: false,
            currency_space: true,
            percent_space: true,
        },
        ("nl", _) => NumberFormatRules {
            decimal_sep: ",",
            thousands_sep: ".",
            grouping: 3,
            currency_symbol: "\u{20ac}",
            currency_prefix: true,
            currency_space: true,
            percent_space: false,
        },
        ("pl", _) => NumberFormatRules {
            decimal_sep: ",",
            thousands_sep: "\u{00a0}",
            grouping: 3,
            currency_symbol: "z\u{0142}",
            currency_prefix: false,
            currency_space: true,
            percent_space: false,
        },
        ("sv", _) => NumberFormatRules {
            decimal_sep: ",",
            thousands_sep: "\u{00a0}",
            grouping: 3,
            currency_symbol: "kr",
            currency_prefix: false,
            currency_space: true,
            percent_space: true,
        },
        ("tr", _) => NumberFormatRules {
            decimal_sep: ",",
            thousands_sep: ".",
            grouping: 3,
            currency_symbol: "\u{20ba}",
            currency_prefix: false,
            currency_space: true,
            percent_space: false,
        },
        _ => NumberFormatRules {
            decimal_sep: ".",
            thousands_sep: ",",
            grouping: 3,
            currency_symbol: "$",
            currency_prefix: true,
            currency_space: false,
            percent_space: false,
        },
    }
}

/// Format the integer part of a number with thousands separators.
fn format_integer_part(digits: &str, thousands_sep: &str, grouping: u8) -> String {
    if grouping == 0 || digits.len() <= grouping as usize {
        return digits.to_string();
    }

    let g = grouping as usize;
    let mut result = String::with_capacity(digits.len() + digits.len() / g);
    let remainder = digits.len() % g;

    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (i - remainder) % g == 0 && (i >= remainder || remainder == 0) {
            // Only insert separator at group boundaries
        }
        result.push(ch);
    }

    // Simpler approach: work from the right
    result.clear();
    let chars: Vec<char> = digits.chars().collect();
    let len = chars.len();
    for (i, &ch) in chars.iter().enumerate() {
        result.push(ch);
        let pos_from_right = len - 1 - i;
        if pos_from_right > 0 && pos_from_right % g == 0 {
            result.push_str(thousands_sep);
        }
    }

    result
}

/// Format a number according to locale conventions.
///
/// Examples:
/// - en_US: `1,234.56`
/// - de_DE: `1.234,56`
/// - fr_FR: `1 234,56`
pub fn format_number(value: f64, locale: &Locale) -> String {
    let rules = rules_for_locale(locale);

    if value.is_nan() {
        return "NaN".to_string();
    }
    if value.is_infinite() {
        return if value > 0.0 { "\u{221e}".to_string() } else { "-\u{221e}".to_string() };
    }

    let negative = value < 0.0;
    let abs_val = value.abs();

    // Format with up to 10 decimal places, then trim trailing zeros
    let formatted = format!("{:.10}", abs_val);
    let (int_part, dec_part) = formatted.split_once('.').unwrap_or((&formatted, ""));

    // Trim trailing zeros from decimal part
    let dec_trimmed = dec_part.trim_end_matches('0');

    let int_formatted = format_integer_part(int_part, rules.thousands_sep, rules.grouping);

    let mut result = String::new();
    if negative {
        result.push('-');
    }
    result.push_str(&int_formatted);
    if !dec_trimmed.is_empty() {
        result.push_str(rules.decimal_sep);
        result.push_str(dec_trimmed);
    }

    result
}

/// Format a currency value according to locale conventions.
///
/// The `currency` parameter is an ISO 4217 code (e.g., "USD", "EUR") which is used
/// to override the default currency symbol when it differs from the locale's default.
///
/// Examples:
/// - en_US, USD: `$1,234.56`
/// - de_DE, EUR: `1.234,56 €`
pub fn format_currency(value: f64, currency: &str, locale: &Locale) -> String {
    let rules = rules_for_locale(locale);

    let symbol = match currency {
        "USD" => "$",
        "EUR" => "\u{20ac}",
        "GBP" => "\u{00a3}",
        "JPY" => "\u{00a5}",
        "CNY" | "RMB" => "\u{00a5}",
        "KRW" => "\u{20a9}",
        "BRL" => "R$",
        "RUB" => "\u{20bd}",
        "INR" => "\u{20b9}",
        "TRY" => "\u{20ba}",
        "PLN" => "z\u{0142}",
        "SEK" | "NOK" | "DKK" => "kr",
        "SAR" => "ر.س",
        "CHF" => "CHF",
        _ => rules.currency_symbol,
    };

    let negative = value < 0.0;
    let abs_val = value.abs();

    // Currency always shows exactly 2 decimal places (except JPY/KRW which show 0)
    let decimals = match currency {
        "JPY" | "KRW" => 0u8,
        _ => 2,
    };

    let (int_part, dec_part) = if decimals == 0 {
        let rounded = abs_val.round() as u64;
        (format!("{}", rounded), String::new())
    } else {
        let formatted = format!("{:.width$}", abs_val, width = decimals as usize);
        let (i, d) = formatted.split_once('.').unwrap();
        (i.to_string(), d.to_string())
    };

    let int_formatted = format_integer_part(&int_part, rules.thousands_sep, rules.grouping);

    let mut number = String::new();
    if negative {
        number.push('-');
    }
    number.push_str(&int_formatted);
    if !dec_part.is_empty() {
        number.push_str(rules.decimal_sep);
        number.push_str(&dec_part);
    }

    // Assemble with currency symbol
    if rules.currency_prefix {
        let mut result = String::new();
        if negative {
            result.push('-');
            result.push_str(symbol);
            if rules.currency_space {
                result.push(' ');
            }
            // Remove leading '-' from number since we put it before the symbol
            result.push_str(number.trim_start_matches('-'));
        } else {
            result.push_str(symbol);
            if rules.currency_space {
                result.push(' ');
            }
            result.push_str(&number);
        }
        result
    } else {
        let mut result = number;
        if rules.currency_space {
            result.push(' ');
        }
        result.push_str(symbol);
        result
    }
}

/// Format a percentage value according to locale conventions.
///
/// Examples:
/// - en_US: `50%`
/// - de_DE: `50 %`
pub fn format_percent(value: f64, locale: &Locale) -> String {
    let rules = rules_for_locale(locale);

    let pct = value * 100.0;
    let negative = pct < 0.0;
    let abs_pct = pct.abs();

    // Show up to 2 decimal places, trim trailing zeros
    let formatted = format!("{:.2}", abs_pct);
    let (int_part, dec_part) = formatted.split_once('.').unwrap_or((&formatted, ""));
    let dec_trimmed = dec_part.trim_end_matches('0');

    let int_formatted = format_integer_part(int_part, rules.thousands_sep, rules.grouping);

    let mut result = String::new();
    if negative {
        result.push('-');
    }
    result.push_str(&int_formatted);
    if !dec_trimmed.is_empty() {
        result.push_str(rules.decimal_sep);
        result.push_str(dec_trimmed);
    }
    if rules.percent_space {
        result.push_str(" %");
    } else {
        result.push('%');
    }

    result
}
