pub mod action;
pub mod binding;
pub mod defaults;
pub mod profile;
pub mod registry;

pub use action::{
    action_category, action_display_name, AppAction, DesktopAction, ShortcutAction, SystemAction,
    WindowAction,
};
pub use binding::{
    KeyBinding, KeyChord, KeyCode, ParseError, MOD_ALT, MOD_CTRL, MOD_HYPER, MOD_NONE, MOD_SHIFT,
    MOD_SUPER,
};
pub use defaults::register_defaults;
pub use profile::{
    apply_profile, export_profile, profile_accessibility, profile_compact, profile_default,
    ShortcutProfile,
};
pub use registry::{
    ConflictError, ShortcutContext, ShortcutEntry, ShortcutRegistry, ShortcutSource,
};
