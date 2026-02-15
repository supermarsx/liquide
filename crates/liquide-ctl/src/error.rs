use thiserror::Error;

/// Exit codes per spec §5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ExitCode {
    Success = 0,
    GeneralError = 1,
    InvalidArguments = 2,
    ConnectionError = 3,
    AuthenticationError = 4,
    PermissionDenied = 5,
    ResourceNotFound = 6,
    OperationCancelled = 7,
    Timeout = 8,
    PluginError = 9,
    SupervisorError = 10,
    CrashReportError = 11,
}

impl From<ExitCode> for std::process::ExitCode {
    fn from(code: ExitCode) -> Self {
        std::process::ExitCode::from(code as u8)
    }
}

impl From<ExitCode> for i32 {
    fn from(code: ExitCode) -> Self {
        code as i32
    }
}

/// Typed errors for liquidctl operations.
#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum LiquidctlError {
    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Authentication error: {0}")]
    Authentication(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Operation cancelled by user")]
    Cancelled,

    #[error("Operation timed out: {0}")]
    Timeout(String),

    #[error("Plugin error: {0}")]
    Plugin(String),

    #[error("Supervisor error: {0}")]
    Supervisor(String),

    #[error("Crash report error: {0}")]
    CrashReport(String),

    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

impl LiquidctlError {
    /// Map error to the appropriate exit code.
    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::Connection(_) => ExitCode::ConnectionError,
            Self::Authentication(_) => ExitCode::AuthenticationError,
            Self::PermissionDenied(_) => ExitCode::PermissionDenied,
            Self::NotFound(_) => ExitCode::ResourceNotFound,
            Self::Cancelled => ExitCode::OperationCancelled,
            Self::Timeout(_) => ExitCode::Timeout,
            Self::Plugin(_) => ExitCode::PluginError,
            Self::Supervisor(_) => ExitCode::SupervisorError,
            Self::CrashReport(_) => ExitCode::CrashReportError,
            Self::InvalidArguments(_) => ExitCode::InvalidArguments,
            Self::Config(_) => ExitCode::GeneralError,
            Self::Other(_) => ExitCode::GeneralError,
        }
    }
}

pub type Result<T> = std::result::Result<T, LiquidctlError>;
