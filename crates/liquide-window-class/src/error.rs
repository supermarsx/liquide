use crate::atom::ClassAtom;
use std::fmt;

/// Errors returned by the class registry operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassError {
    /// A class with this name is already registered in the same scope.
    AlreadyRegistered { name: String },
    /// The referenced atom does not exist in the registry.
    NotFound { atom: ClassAtom },
    /// Cannot unregister a system class.
    SystemClass { atom: ClassAtom },
    /// Cannot unregister while windows of this class still exist.
    WindowsExist { atom: ClassAtom, count: usize },
    /// The class name is empty.
    EmptyName,
    /// The field index passed to `set_class_long` is invalid.
    InvalidField { field: i32 },
}

impl fmt::Display for ClassError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyRegistered { name } => {
                write!(f, "class '{name}' is already registered")
            }
            Self::NotFound { atom } => {
                write!(f, "class {atom} not found")
            }
            Self::SystemClass { atom } => {
                write!(f, "cannot unregister system class {atom}")
            }
            Self::WindowsExist { atom, count } => {
                write!(
                    f,
                    "cannot unregister class {atom}: {count} window(s) still exist"
                )
            }
            Self::EmptyName => write!(f, "class name must not be empty"),
            Self::InvalidField { field } => {
                write!(f, "invalid class field index: {field}")
            }
        }
    }
}

impl std::error::Error for ClassError {}
