/// Timezone descriptor with IANA ID, display info, and UTC offset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeZone {
    /// IANA timezone identifier, e.g. "America/New_York".
    pub id: String,
    /// Human-readable display name, e.g. "Eastern Time (US & Canada)".
    pub display_name: String,
    /// Current UTC offset in minutes. For example, UTC-5 = -300.
    pub utc_offset_minutes: i32,
    /// Abbreviation, e.g. "EST", "CET".
    pub abbreviation: String,
    /// Whether this timezone observes daylight saving time.
    pub uses_dst: bool,
}

impl TimeZone {
    /// Create a new timezone entry.
    pub fn new(
        id: impl Into<String>,
        display_name: impl Into<String>,
        utc_offset_minutes: i32,
        abbreviation: impl Into<String>,
        uses_dst: bool,
    ) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            utc_offset_minutes,
            abbreviation: abbreviation.into(),
            uses_dst,
        }
    }

    /// Format the UTC offset as "+HH:MM" or "-HH:MM".
    pub fn format_offset(&self) -> String {
        let sign = if self.utc_offset_minutes >= 0 { '+' } else { '-' };
        let abs = self.utc_offset_minutes.unsigned_abs();
        let h = abs / 60;
        let m = abs % 60;
        format!("UTC{}{:02}:{:02}", sign, h, m)
    }
}

impl std::fmt::Display for TimeZone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({}, {})", self.display_name, self.abbreviation, self.format_offset())
    }
}
