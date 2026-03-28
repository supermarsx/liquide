use crate::*;

// ── Locale parsing ──────────────────────────────────────────────

#[test]
fn parse_full_locale() {
    let loc = Locale::parse("en_US.UTF-8").unwrap();
    assert_eq!(loc.language, "en");
    assert_eq!(loc.territory.as_deref(), Some("US"));
    assert_eq!(loc.encoding.as_deref(), Some("UTF-8"));
}

#[test]
fn parse_language_territory() {
    let loc = Locale::parse("de_DE").unwrap();
    assert_eq!(loc.language, "de");
    assert_eq!(loc.territory.as_deref(), Some("DE"));
    assert_eq!(loc.encoding, None);
}

#[test]
fn parse_language_only() {
    let loc = Locale::parse("ja").unwrap();
    assert_eq!(loc.language, "ja");
    assert_eq!(loc.territory, None);
    assert_eq!(loc.encoding, None);
}

#[test]
fn parse_with_hyphen() {
    let loc = Locale::parse("pt-BR").unwrap();
    assert_eq!(loc.language, "pt");
    assert_eq!(loc.territory.as_deref(), Some("BR"));
}

#[test]
fn parse_with_modifier() {
    let loc = Locale::parse("sr_RS.UTF-8@latin").unwrap();
    assert_eq!(loc.language, "sr");
    assert_eq!(loc.territory.as_deref(), Some("RS"));
    assert_eq!(loc.encoding.as_deref(), Some("UTF-8"));
}

#[test]
fn parse_posix() {
    let loc = Locale::parse("C").unwrap();
    assert_eq!(loc.language, "en");

    let loc2 = Locale::parse("POSIX").unwrap();
    assert_eq!(loc2.language, "en");
}

#[test]
fn parse_empty_returns_none() {
    assert!(Locale::parse("").is_none());
}

#[test]
fn parse_invalid_returns_none() {
    assert!(Locale::parse("1234").is_none());
    assert!(Locale::parse("toolong_XX").is_none());
}

#[test]
fn locale_display() {
    let loc = Locale::with_encoding("en", "US", "UTF-8");
    assert_eq!(loc.to_string(), "en_US.UTF-8");

    let loc2 = Locale::with_territory("de", "DE");
    assert_eq!(loc2.to_string(), "de_DE");

    let loc3 = Locale::new("fr");
    assert_eq!(loc3.to_string(), "fr");
}

#[test]
fn language_name() {
    let en = Locale::new("en");
    assert_eq!(en.language_name(), "English");

    let ja = Locale::new("ja");
    assert_eq!(ja.language_name(), "Japanese");

    let de = Locale::new("de");
    assert_eq!(de.language_name(), "German");

    let unknown = Locale::new("xx");
    assert_eq!(unknown.language_name(), "Unknown");
}

// ── LocaleManager ───────────────────────────────────────────────

#[test]
fn manager_default_locale() {
    let mgr = LocaleManager::with_locale(Locale::with_territory("fr", "FR"));
    assert_eq!(mgr.active_locale().language, "fr");
}

#[test]
fn manager_set_locale() {
    let mut mgr = LocaleManager::new();
    let ja = Locale::with_territory("ja", "JP");
    mgr.set_locale(&ja);
    assert_eq!(mgr.active_locale().language, "ja");
}

#[test]
fn available_locales_non_empty() {
    let mgr = LocaleManager::new();
    let locales = mgr.available_locales();
    assert!(locales.len() >= 10);
}

// ── Number formatting ───────────────────────────────────────────

#[test]
fn format_number_en_us() {
    let loc = Locale::with_territory("en", "US");
    assert_eq!(format_number(1234.56, &loc), "1,234.56");
}

#[test]
fn format_number_de_de() {
    let loc = Locale::with_territory("de", "DE");
    assert_eq!(format_number(1234.56, &loc), "1.234,56");
}

#[test]
fn format_number_negative() {
    let loc = Locale::with_territory("en", "US");
    assert_eq!(format_number(-42.5, &loc), "-42.5");
}

#[test]
fn format_number_zero() {
    let loc = Locale::with_territory("en", "US");
    assert_eq!(format_number(0.0, &loc), "0");
}

#[test]
fn format_number_large() {
    let loc = Locale::with_territory("en", "US");
    assert_eq!(format_number(1000000.0, &loc), "1,000,000");
}

#[test]
fn format_currency_usd() {
    let loc = Locale::with_territory("en", "US");
    assert_eq!(format_currency(1234.56, "USD", &loc), "$1,234.56");
}

#[test]
fn format_currency_eur_de() {
    let loc = Locale::with_territory("de", "DE");
    let result = format_currency(1234.56, "EUR", &loc);
    assert!(result.contains("1.234,56"));
    assert!(result.contains('\u{20ac}'));
}

#[test]
fn format_currency_jpy() {
    let loc = Locale::with_territory("ja", "JP");
    let result = format_currency(1234.0, "JPY", &loc);
    // JPY has 0 decimal places
    assert!(result.contains("1,234"));
    assert!(!result.contains('.'));
}

#[test]
fn format_percent_en() {
    let loc = Locale::with_territory("en", "US");
    assert_eq!(format_percent(0.5, &loc), "50%");
    assert_eq!(format_percent(1.0, &loc), "100%");
}

#[test]
fn format_percent_de() {
    let loc = Locale::with_territory("de", "DE");
    assert_eq!(format_percent(0.5, &loc), "50 %");
}

// ── Date formatting ─────────────────────────────────────────────

#[test]
fn format_date_iso() {
    let loc = Locale::with_territory("en", "US");
    assert_eq!(
        format_date(2024, 1, 15, &loc, DateStyle::ISO),
        "2024-01-15"
    );
}

#[test]
fn format_date_short_us() {
    let loc = Locale::with_territory("en", "US");
    assert_eq!(
        format_date(2024, 1, 15, &loc, DateStyle::Short),
        "1/15/24"
    );
}

#[test]
fn format_date_short_de() {
    let loc = Locale::with_territory("de", "DE");
    assert_eq!(
        format_date(2024, 1, 15, &loc, DateStyle::Short),
        "15.1.24"
    );
}

#[test]
fn format_date_medium_en() {
    let loc = Locale::with_territory("en", "US");
    assert_eq!(
        format_date(2024, 1, 15, &loc, DateStyle::Medium),
        "Jan 15, 2024"
    );
}

#[test]
fn format_date_long_en() {
    let loc = Locale::with_territory("en", "US");
    assert_eq!(
        format_date(2024, 1, 15, &loc, DateStyle::Long),
        "January 15, 2024"
    );
}

#[test]
fn format_date_long_de() {
    let loc = Locale::with_territory("de", "DE");
    assert_eq!(
        format_date(2024, 3, 5, &loc, DateStyle::Long),
        "5. M\u{00e4}rz 2024"
    );
}

#[test]
fn format_date_long_es() {
    let loc = Locale::with_territory("es", "ES");
    assert_eq!(
        format_date(2024, 1, 15, &loc, DateStyle::Long),
        "15 de enero de 2024"
    );
}

#[test]
fn format_time_short_en() {
    let loc = Locale::with_territory("en", "US");
    assert_eq!(
        format_time(15, 30, 0, &loc, TimeStyle::Short),
        "3:30 PM"
    );
    assert_eq!(
        format_time(9, 5, 0, &loc, TimeStyle::Short),
        "9:05 AM"
    );
}

#[test]
fn format_time_h24() {
    let loc = Locale::with_territory("de", "DE");
    assert_eq!(
        format_time(15, 30, 0, &loc, TimeStyle::H24),
        "15:30"
    );
}

#[test]
fn format_time_long() {
    let loc = Locale::with_territory("de", "DE");
    assert_eq!(
        format_time(15, 30, 45, &loc, TimeStyle::Long),
        "15:30:45"
    );
}

#[test]
fn format_time_midnight() {
    let loc = Locale::with_territory("en", "US");
    assert_eq!(
        format_time(0, 0, 0, &loc, TimeStyle::Short),
        "12:00 AM"
    );
}

#[test]
fn month_names_english() {
    let loc = Locale::with_territory("en", "US");
    assert_eq!(month_name(1, &loc), "January");
    assert_eq!(month_name(12, &loc), "December");
    assert_eq!(month_name_short(3, &loc), "Mar");
}

#[test]
fn month_names_french() {
    let loc = Locale::new("fr");
    assert_eq!(month_name(1, &loc), "janvier");
    assert_eq!(month_name(7, &loc), "juillet");
}

#[test]
fn day_names_english() {
    let loc = Locale::with_territory("en", "US");
    assert_eq!(day_name(0, &loc), "Sunday");
    assert_eq!(day_name(1, &loc), "Monday");
    assert_eq!(day_name_short(5, &loc), "Fri");
}

// ── Translation catalog ─────────────────────────────────────────

#[test]
fn catalog_basic() {
    let mut cat = Catalog::new(Locale::new("en"));
    cat.add_translation("hello", "Hello!");
    cat.add_translation("bye", "Goodbye!");

    assert_eq!(cat.get("hello"), "Hello!");
    assert_eq!(cat.get("bye"), "Goodbye!");
    assert_eq!(cat.get("missing"), "missing"); // fallback to key
}

#[test]
fn catalog_plural() {
    let mut cat = Catalog::new(Locale::new("en"));
    cat.add_translation("items.one", "1 item");
    cat.add_translation("items.other", "{count} items");

    assert_eq!(cat.get_plural("items", 1), "1 item");
    assert_eq!(cat.get_plural("items", 5), "{count} items");
    assert_eq!(cat.get_plural("items", 0), "{count} items");
}

#[test]
fn catalog_load_from_string() {
    let data = "# Comment\nhello = Hallo\nbye = Tsch\u{00fc}ss\n\nitems.one = 1 Element\nitems.other = Elemente\n";
    let cat = Catalog::load_from_string(data, Locale::new("de")).unwrap();

    assert_eq!(cat.get("hello"), "Hallo");
    assert_eq!(cat.get("bye"), "Tsch\u{00fc}ss");
    assert_eq!(cat.len(), 4);
}

#[test]
fn catalog_load_error() {
    let data = "no equals sign here";
    let result = Catalog::load_from_string(data, Locale::new("en"));
    assert!(result.is_err());
}

#[test]
fn catalog_merge() {
    let mut base = Catalog::new(Locale::new("en"));
    base.add_translation("hello", "Hello");
    base.add_translation("bye", "Bye");

    let mut overlay = Catalog::new(Locale::new("en"));
    overlay.add_translation("hello", "Hi there!");
    overlay.add_translation("new_key", "New");

    base.merge(&overlay);

    assert_eq!(base.get("hello"), "Hi there!"); // overridden
    assert_eq!(base.get("bye"), "Bye"); // preserved
    assert_eq!(base.get("new_key"), "New"); // added
}

#[test]
fn catalog_contains_and_empty() {
    let cat = Catalog::new(Locale::new("en"));
    assert!(cat.is_empty());
    assert!(!cat.contains_key("hello"));
}

// ── Text direction ──────────────────────────────────────────────

#[test]
fn text_direction_ltr() {
    assert_eq!(text_direction(&Locale::new("en")), TextDir::LTR);
    assert_eq!(text_direction(&Locale::new("de")), TextDir::LTR);
    assert_eq!(text_direction(&Locale::new("ja")), TextDir::LTR);
    assert_eq!(text_direction(&Locale::new("zh")), TextDir::LTR);
}

#[test]
fn text_direction_rtl() {
    assert_eq!(text_direction(&Locale::new("ar")), TextDir::RTL);
    assert_eq!(text_direction(&Locale::new("he")), TextDir::RTL);
    assert_eq!(text_direction(&Locale::new("fa")), TextDir::RTL);
    assert_eq!(text_direction(&Locale::new("ur")), TextDir::RTL);
    assert_eq!(text_direction(&Locale::new("yi")), TextDir::RTL);
}

// ── Calendar system ─────────────────────────────────────────────

#[test]
fn calendar_gregorian() {
    let loc = Locale::with_territory("en", "US");
    assert_eq!(calendar_for_locale(&loc), CalendarSystem::Gregorian);
}

#[test]
fn calendar_japanese() {
    let loc = Locale::with_territory("ja", "JP");
    assert_eq!(calendar_for_locale(&loc), CalendarSystem::Japanese);

    let (era, year) = CalendarSystem::Japanese.convert_year(2024);
    assert_eq!(era, "Reiwa");
    assert_eq!(year, 6);
}

#[test]
fn calendar_japanese_heisei() {
    let (era, year) = CalendarSystem::Japanese.convert_year(1995);
    assert_eq!(era, "Heisei");
    assert_eq!(year, 7);
}

#[test]
fn calendar_buddhist() {
    let loc = Locale::with_territory("th", "TH");
    assert_eq!(calendar_for_locale(&loc), CalendarSystem::Buddhist);

    let (era, year) = CalendarSystem::Buddhist.convert_year(2024);
    assert_eq!(era, "BE");
    assert_eq!(year, 2567);
}

#[test]
fn calendar_islamic() {
    let loc = Locale::with_territory("ar", "SA");
    assert_eq!(calendar_for_locale(&loc), CalendarSystem::Islamic);

    let (era, year) = CalendarSystem::Islamic.convert_year(2024);
    assert_eq!(era, "AH");
    assert!(year >= 1445 && year <= 1446); // approximate
}

// ── Measurement system ──────────────────────────────────────────

#[test]
fn measurement_metric() {
    let loc = Locale::with_territory("de", "DE");
    assert_eq!(measurement_for_locale(&loc), MeasurementSystem::Metric);
    assert_eq!(MeasurementSystem::Metric.temperature_unit(), "\u{00b0}C");
}

#[test]
fn measurement_us_customary() {
    let loc = Locale::with_territory("en", "US");
    assert_eq!(measurement_for_locale(&loc), MeasurementSystem::USCustomary);
    assert_eq!(MeasurementSystem::USCustomary.temperature_unit(), "\u{00b0}F");
}

#[test]
fn measurement_imperial() {
    let loc = Locale::with_territory("en", "GB");
    assert_eq!(measurement_for_locale(&loc), MeasurementSystem::Imperial);
}

// ── Error display ───────────────────────────────────────────────

#[test]
fn error_display() {
    let e = LocaleError::InvalidLocale("bad".into());
    assert_eq!(e.to_string(), "invalid locale: bad");

    let e2 = LocaleError::ParseError("line 3".into());
    assert_eq!(e2.to_string(), "parse error: line 3");
}

// ── Round-trip ──────────────────────────────────────────────────

#[test]
fn locale_parse_roundtrip() {
    for s in &["en_US.UTF-8", "de_DE", "ja", "fr_FR.ISO-8859-1", "zh_CN.UTF-8"] {
        let loc = Locale::parse(s).unwrap();
        let formatted = loc.to_string();
        let reparsed = Locale::parse(&formatted).unwrap();
        assert_eq!(loc, reparsed);
    }
}

#[test]
fn format_number_fr_fr() {
    let loc = Locale::with_territory("fr", "FR");
    let result = format_number(1234.56, &loc);
    // French uses comma decimal, narrow no-break space thousands
    assert!(result.contains(','));
    assert!(result.contains("234"));
}

#[test]
fn format_currency_gbp() {
    let loc = Locale::with_territory("en", "GB");
    let result = format_currency(99.99, "GBP", &loc);
    assert!(result.contains('\u{00a3}'));
    assert!(result.contains("99.99"));
}
