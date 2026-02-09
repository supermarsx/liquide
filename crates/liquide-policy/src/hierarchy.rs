//! Policy hierarchy definition and layer ordering.

use crate::PolicySource;

/// Describes the full hierarchy for policy resolution.
///
/// Layers are always evaluated from lowest priority (**Server**) to
/// highest priority (**Session**).
pub const HIERARCHY_ORDER: &[PolicySource] = &[
    PolicySource::Server,
    PolicySource::Group,
    PolicySource::User,
    PolicySource::Session,
];

/// Check whether `higher` may override `lower` in the policy hierarchy.
#[must_use]
pub fn can_override(lower: PolicySource, higher: PolicySource) -> bool {
    higher > lower
}
