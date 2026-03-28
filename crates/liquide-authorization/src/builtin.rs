use crate::action::AuthorizationAction;
use crate::level::AuthLevel;

/// Returns the set of built-in privileged actions known to the Liquide desktop.
///
/// These cover common system operations that may require privilege escalation.
/// Applications and plugins can register additional actions at runtime.
#[must_use]
pub fn builtin_actions() -> Vec<AuthorizationAction> {
    vec![
        // ── System power ────────────────────────────────────────────
        AuthorizationAction::new(
            "org.liquide.system.shutdown",
            "Shut down the system",
            "The system will shut down. Unsaved work may be lost.",
            AuthLevel::NoAuth,
        )
        .with_icon("system-shutdown"),
        AuthorizationAction::new(
            "org.liquide.system.reboot",
            "Restart the system",
            "The system will restart. Unsaved work may be lost.",
            AuthLevel::NoAuth,
        )
        .with_icon("system-reboot"),
        AuthorizationAction::new(
            "org.liquide.system.suspend",
            "Suspend the system",
            "The system will be suspended to RAM.",
            AuthLevel::NoAuth,
        )
        .with_icon("system-suspend"),
        // ── Package management ──────────────────────────────────────
        AuthorizationAction::new(
            "org.liquide.package.install",
            "Install software",
            "Authentication is required to install software packages.",
            AuthLevel::AdminPassword,
        )
        .with_icon("package-install"),
        AuthorizationAction::new(
            "org.liquide.package.remove",
            "Remove software",
            "Authentication is required to remove software packages.",
            AuthLevel::AdminPassword,
        )
        .with_icon("package-remove"),
        AuthorizationAction::new(
            "org.liquide.package.update",
            "Update software",
            "Authentication is required to update software packages.",
            AuthLevel::AdminPassword,
        )
        .with_icon("package-upgrade"),
        // ── System settings ─────────────────────────────────────────
        AuthorizationAction::new(
            "org.liquide.settings.system.time",
            "Change system time and date",
            "Authentication is required to change the system clock.",
            AuthLevel::UserPassword,
        )
        .with_icon("preferences-system-time"),
        AuthorizationAction::new(
            "org.liquide.settings.system.network",
            "Modify network configuration",
            "Authentication is required to change network settings.",
            AuthLevel::UserPassword,
        )
        .with_icon("preferences-system-network"),
        AuthorizationAction::new(
            "org.liquide.settings.system.users",
            "Manage user accounts",
            "Authentication is required to manage user accounts.",
            AuthLevel::AdminPassword,
        )
        .with_icon("system-users"),
        // ── Device management ───────────────────────────────────────
        AuthorizationAction::new(
            "org.liquide.device.mount",
            "Mount a device",
            "Authentication is required to mount this device.",
            AuthLevel::UserPassword,
        )
        .with_icon("drive-harddisk"),
        AuthorizationAction::new(
            "org.liquide.device.unmount",
            "Unmount a device",
            "Authentication is required to safely unmount this device.",
            AuthLevel::UserPassword,
        )
        .with_icon("media-eject"),
        // ── Service management ──────────────────────────────────────
        AuthorizationAction::new(
            "org.liquide.service.start",
            "Start a system service",
            "Authentication is required to start this system service.",
            AuthLevel::AdminPassword,
        )
        .with_icon("system-run"),
        AuthorizationAction::new(
            "org.liquide.service.stop",
            "Stop a system service",
            "Authentication is required to stop this system service.",
            AuthLevel::AdminPassword,
        )
        .with_icon("process-stop"),
    ]
}

/// Look up a builtin action by its ID.
///
/// Returns `None` if the ID does not match any builtin action.
#[must_use]
pub fn find_builtin(action_id: &str) -> Option<AuthorizationAction> {
    builtin_actions().into_iter().find(|a| a.id == action_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_count() {
        let actions = builtin_actions();
        assert_eq!(actions.len(), 13);
    }

    #[test]
    fn all_have_icons() {
        for action in builtin_actions() {
            assert!(
                action.icon.is_some(),
                "builtin action {} has no icon",
                action.id
            );
        }
    }

    #[test]
    fn all_have_descriptions() {
        for action in builtin_actions() {
            assert!(
                !action.description.is_empty(),
                "builtin action {} has empty description",
                action.id
            );
        }
    }

    #[test]
    fn all_have_messages() {
        for action in builtin_actions() {
            assert!(
                !action.message.is_empty(),
                "builtin action {} has empty message",
                action.id
            );
        }
    }

    #[test]
    fn unique_ids() {
        let actions = builtin_actions();
        let mut ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), actions.len(), "duplicate action IDs found");
    }

    #[test]
    fn system_power_is_noauth() {
        for action in builtin_actions() {
            if action.id.starts_with("org.liquide.system.") {
                assert_eq!(
                    action.required_level,
                    AuthLevel::NoAuth,
                    "{} should be NoAuth",
                    action.id
                );
            }
        }
    }

    #[test]
    fn package_actions_are_admin() {
        for action in builtin_actions() {
            if action.id.starts_with("org.liquide.package.") {
                assert_eq!(
                    action.required_level,
                    AuthLevel::AdminPassword,
                    "{} should be AdminPassword",
                    action.id
                );
            }
        }
    }

    #[test]
    fn find_builtin_hit() {
        let action = find_builtin("org.liquide.system.shutdown");
        assert!(action.is_some());
        assert_eq!(action.unwrap().id, "org.liquide.system.shutdown");
    }

    #[test]
    fn find_builtin_miss() {
        assert!(find_builtin("org.liquide.nonexistent").is_none());
    }
}
