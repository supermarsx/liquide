pub mod action;
pub mod binding;
pub mod defaults;
pub mod profile;
pub mod registry;

pub use action::{
    AppAction, DesktopAction, ShortcutAction, SystemAction, WindowAction, action_category,
    action_display_name,
};
pub use binding::{
    KeyBinding, KeyChord, KeyCode, MOD_ALT, MOD_CTRL, MOD_HYPER, MOD_NONE, MOD_SHIFT, MOD_SUPER,
    ParseError,
};
pub use defaults::register_defaults;
pub use profile::{
    ShortcutProfile, apply_profile, export_profile, profile_accessibility, profile_compact,
    profile_default,
};
pub use registry::{
    ConflictError, ShortcutContext, ShortcutEntry, ShortcutRegistry, ShortcutSource,
};
