//! Policy evaluation logic.

use crate::rule::{RuleAction, RuleSet};
use crate::{EffectivePolicy, PolicySource};

/// Evaluate the provided layers into an [`EffectivePolicy`].
///
/// Rules are applied in source-priority order: server-wide defaults first,
/// then group, user, and session overrides.
pub fn evaluate(layers: &[(PolicySource, RuleSet)]) -> EffectivePolicy {
    let mut policy = EffectivePolicy {
        clipboard_enabled: true,
        usb_redirect_enabled: false,
        audio_playback_enabled: true,
        audio_capture_enabled: false,
        file_transfer_enabled: true,
        printing_enabled: true,
        max_resolution_w: 3840,
        max_resolution_h: 2160,
        idle_timeout_secs: 0,
    };

    for (_source, ruleset) in layers {
        for rule in &ruleset.rules {
            apply_rule(&mut policy, &rule.key, &rule.action);
        }
    }

    policy
}

/// Apply a single rule to the effective policy.
fn apply_rule(policy: &mut EffectivePolicy, key: &str, action: &RuleAction) {
    match key {
        "clipboard.enabled" => {
            policy.clipboard_enabled = matches!(action, RuleAction::Allow);
        }
        "usb_redirect.enabled" => {
            policy.usb_redirect_enabled = matches!(action, RuleAction::Allow);
        }
        "audio.playback" => {
            policy.audio_playback_enabled = matches!(action, RuleAction::Allow);
        }
        "audio.capture" => {
            policy.audio_capture_enabled = matches!(action, RuleAction::Allow);
        }
        "file_transfer.enabled" => {
            policy.file_transfer_enabled = matches!(action, RuleAction::Allow);
        }
        "printing.enabled" => {
            policy.printing_enabled = matches!(action, RuleAction::Allow);
        }
        "display.max_width" => {
            if let RuleAction::Set(v) = action {
                if let Ok(w) = v.parse() {
                    policy.max_resolution_w = w;
                }
            }
        }
        "display.max_height" => {
            if let RuleAction::Set(v) = action {
                if let Ok(h) = v.parse() {
                    policy.max_resolution_h = h;
                }
            }
        }
        "session.idle_timeout" => {
            if let RuleAction::Set(v) = action {
                if let Ok(t) = v.parse() {
                    policy.idle_timeout_secs = t;
                }
            }
        }
        _ => {
            tracing::warn!(key, "unknown policy key — ignoring");
        }
    }
}
