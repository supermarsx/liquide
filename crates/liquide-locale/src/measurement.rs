use crate::locale::Locale;

/// Measurement system used by a locale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MeasurementSystem {
    /// Metric system (SI units) — used by most countries.
    Metric,
    /// Imperial system — used by the UK for some measurements.
    Imperial,
    /// US customary system — used by the United States.
    USCustomary,
}

impl MeasurementSystem {
    /// Human-readable name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Metric => "Metric",
            Self::Imperial => "Imperial",
            Self::USCustomary => "US Customary",
        }
    }

    /// The standard unit for temperature in this system.
    pub fn temperature_unit(&self) -> &'static str {
        match self {
            Self::Metric => "\u{00b0}C",
            Self::Imperial | Self::USCustomary => "\u{00b0}F",
        }
    }

    /// The standard unit for distance in this system.
    pub fn distance_unit(&self) -> &'static str {
        match self {
            Self::Metric => "km",
            Self::Imperial | Self::USCustomary => "mi",
        }
    }

    /// The standard unit for weight in this system.
    pub fn weight_unit(&self) -> &'static str {
        match self {
            Self::Metric => "kg",
            Self::Imperial => "st",    // stone (UK)
            Self::USCustomary => "lb", // pounds (US)
        }
    }
}

/// Determine the measurement system for a locale.
pub fn measurement_for_locale(locale: &Locale) -> MeasurementSystem {
    match locale.territory.as_deref() {
        Some("US") => MeasurementSystem::USCustomary,
        Some("GB") => MeasurementSystem::Imperial,
        Some("MM") => MeasurementSystem::Imperial, // Myanmar
        Some("LR") => MeasurementSystem::USCustomary, // Liberia
        _ => MeasurementSystem::Metric,
    }
}
