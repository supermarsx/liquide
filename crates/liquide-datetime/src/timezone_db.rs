use crate::timezone::TimeZone;

/// Built-in database of major world timezones.
///
/// Contains 50+ commonly-used IANA timezones with their standard UTC offsets.
/// Note: offsets represent the standard (non-DST) offset. DST shifts should be
/// handled by the platform bridge or an external source.
pub struct TimeZoneDatabase {
    zones: Vec<TimeZone>,
}

impl TimeZoneDatabase {
    /// Create the database with all built-in timezones.
    pub fn new() -> Self {
        Self {
            zones: build_timezone_table(),
        }
    }

    /// Return all timezones sorted by UTC offset.
    pub fn all_timezones(&self) -> Vec<&TimeZone> {
        let mut v: Vec<&TimeZone> = self.zones.iter().collect();
        v.sort_by_key(|tz| tz.utc_offset_minutes);
        v
    }

    /// Find a timezone by exact IANA ID (case-insensitive).
    pub fn find(&self, id: &str) -> Option<&TimeZone> {
        let lower = id.to_ascii_lowercase();
        self.zones
            .iter()
            .find(|tz| tz.id.to_ascii_lowercase() == lower)
    }

    /// Search timezones by query string. Matches against ID, display name,
    /// and abbreviation (case-insensitive). Returns all matches.
    pub fn search(&self, query: &str) -> Vec<&TimeZone> {
        let q = query.to_ascii_lowercase();
        self.zones
            .iter()
            .filter(|tz| {
                tz.id.to_ascii_lowercase().contains(&q)
                    || tz.display_name.to_ascii_lowercase().contains(&q)
                    || tz.abbreviation.to_ascii_lowercase().contains(&q)
            })
            .collect()
    }

    /// Find all timezones with the given UTC offset (in minutes).
    pub fn by_offset(&self, offset_minutes: i32) -> Vec<&TimeZone> {
        self.zones
            .iter()
            .filter(|tz| tz.utc_offset_minutes == offset_minutes)
            .collect()
    }

    /// Number of timezones in the database.
    pub fn len(&self) -> usize {
        self.zones.len()
    }

    /// Returns true if the database is empty (always false for default).
    pub fn is_empty(&self) -> bool {
        self.zones.is_empty()
    }
}

impl Default for TimeZoneDatabase {
    fn default() -> Self {
        Self::new()
    }
}

fn tz(id: &str, display: &str, offset: i32, abbr: &str, dst: bool) -> TimeZone {
    TimeZone::new(id, display, offset, abbr, dst)
}

fn build_timezone_table() -> Vec<TimeZone> {
    vec![
        // UTC-12 to UTC-9
        tz("Pacific/Baker", "Baker Island", -720, "BIT", false),
        tz("Pacific/Pago_Pago", "American Samoa", -660, "SST", false),
        tz("Pacific/Honolulu", "Hawaii", -600, "HST", false),
        tz("America/Anchorage", "Alaska", -540, "AKST", true),
        // UTC-8 to UTC-5
        tz(
            "America/Los_Angeles",
            "Pacific Time (US & Canada)",
            -480,
            "PST",
            true,
        ),
        tz("America/Vancouver", "Vancouver", -480, "PST", true),
        tz("America/Tijuana", "Tijuana", -480, "PST", true),
        tz(
            "America/Denver",
            "Mountain Time (US & Canada)",
            -420,
            "MST",
            true,
        ),
        tz("America/Phoenix", "Arizona", -420, "MST", false),
        tz("America/Edmonton", "Edmonton", -420, "MST", true),
        tz(
            "America/Chicago",
            "Central Time (US & Canada)",
            -360,
            "CST",
            true,
        ),
        tz("America/Mexico_City", "Mexico City", -360, "CST", true),
        tz("America/Winnipeg", "Winnipeg", -360, "CST", true),
        tz(
            "America/New_York",
            "Eastern Time (US & Canada)",
            -300,
            "EST",
            true,
        ),
        tz("America/Toronto", "Toronto", -300, "EST", true),
        tz("America/Bogota", "Bogota", -300, "COT", false),
        tz("America/Lima", "Lima", -300, "PET", false),
        // UTC-4 to UTC-3
        tz(
            "America/Halifax",
            "Atlantic Time (Canada)",
            -240,
            "AST",
            true,
        ),
        tz("America/Caracas", "Caracas", -240, "VET", false),
        tz("America/Santiago", "Santiago", -240, "CLT", true),
        tz("America/St_Johns", "Newfoundland", -210, "NST", true),
        tz("America/Sao_Paulo", "Brasilia", -180, "BRT", false),
        tz(
            "America/Argentina/Buenos_Aires",
            "Buenos Aires",
            -180,
            "ART",
            false,
        ),
        // UTC-2 to UTC+0
        tz(
            "Atlantic/South_Georgia",
            "South Georgia",
            -120,
            "GST",
            false,
        ),
        tz("Atlantic/Azores", "Azores", -60, "AZOT", true),
        tz("Atlantic/Cape_Verde", "Cape Verde", -60, "CVT", false),
        tz("UTC", "Coordinated Universal Time", 0, "UTC", false),
        tz("Europe/London", "London", 0, "GMT", true),
        tz("Africa/Casablanca", "Casablanca", 0, "WET", true),
        tz("Africa/Accra", "Accra", 0, "GMT", false),
        // UTC+1 to UTC+3
        tz("Europe/Paris", "Paris", 60, "CET", true),
        tz("Europe/Berlin", "Berlin", 60, "CET", true),
        tz("Europe/Amsterdam", "Amsterdam", 60, "CET", true),
        tz("Europe/Brussels", "Brussels", 60, "CET", true),
        tz("Europe/Madrid", "Madrid", 60, "CET", true),
        tz("Europe/Rome", "Rome", 60, "CET", true),
        tz("Europe/Zurich", "Zurich", 60, "CET", true),
        tz("Europe/Warsaw", "Warsaw", 60, "CET", true),
        tz("Africa/Lagos", "Lagos", 60, "WAT", false),
        tz("Europe/Athens", "Athens", 120, "EET", true),
        tz("Europe/Bucharest", "Bucharest", 120, "EET", true),
        tz("Europe/Helsinki", "Helsinki", 120, "EET", true),
        tz("Europe/Istanbul", "Istanbul", 180, "TRT", false),
        tz("Africa/Cairo", "Cairo", 120, "EET", false),
        tz("Asia/Jerusalem", "Jerusalem", 120, "IST", true),
        tz("Europe/Moscow", "Moscow", 180, "MSK", false),
        tz("Africa/Nairobi", "Nairobi", 180, "EAT", false),
        tz("Asia/Riyadh", "Riyadh", 180, "AST", false),
        // UTC+3:30 to UTC+5:30
        tz("Asia/Tehran", "Tehran", 210, "IRST", true),
        tz("Asia/Dubai", "Dubai", 240, "GST", false),
        tz("Asia/Baku", "Baku", 240, "AZT", true),
        tz("Asia/Kabul", "Kabul", 270, "AFT", false),
        tz("Asia/Karachi", "Karachi", 300, "PKT", false),
        tz("Asia/Tashkent", "Tashkent", 300, "UZT", false),
        tz("Asia/Kolkata", "Kolkata", 330, "IST", false),
        tz("Asia/Kathmandu", "Kathmandu", 345, "NPT", false),
        // UTC+6 to UTC+9
        tz("Asia/Dhaka", "Dhaka", 360, "BST", false),
        tz("Asia/Almaty", "Almaty", 360, "ALMT", false),
        tz("Asia/Yangon", "Yangon", 390, "MMT", false),
        tz("Asia/Bangkok", "Bangkok", 420, "ICT", false),
        tz("Asia/Jakarta", "Jakarta", 420, "WIB", false),
        tz("Asia/Ho_Chi_Minh", "Ho Chi Minh City", 420, "ICT", false),
        tz("Asia/Shanghai", "China Standard Time", 480, "CST", false),
        tz("Asia/Hong_Kong", "Hong Kong", 480, "HKT", false),
        tz("Asia/Singapore", "Singapore", 480, "SGT", false),
        tz("Asia/Taipei", "Taipei", 480, "CST", false),
        tz("Australia/Perth", "Perth", 480, "AWST", false),
        tz("Asia/Seoul", "Seoul", 540, "KST", false),
        tz("Asia/Tokyo", "Tokyo", 540, "JST", false),
        // UTC+9:30 to UTC+12
        tz("Australia/Adelaide", "Adelaide", 570, "ACST", true),
        tz("Australia/Darwin", "Darwin", 570, "ACST", false),
        tz("Australia/Sydney", "Sydney", 600, "AEST", true),
        tz("Australia/Brisbane", "Brisbane", 600, "AEST", false),
        tz("Australia/Melbourne", "Melbourne", 600, "AEST", true),
        tz("Pacific/Guam", "Guam", 600, "ChST", false),
        tz("Pacific/Noumea", "Noumea", 660, "NCT", false),
        tz("Pacific/Auckland", "Auckland", 720, "NZST", true),
        tz("Pacific/Fiji", "Fiji", 720, "FJT", true),
        tz("Pacific/Tongatapu", "Nuku'alofa", 780, "TOT", false),
    ]
}
