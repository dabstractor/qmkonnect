use std::path::PathBuf;
use thiserror::Error;

/// Comprehensive error types for qmkonnect application
#[derive(Debug, Error)]
pub enum QMKError {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("Device communication failed: {0}")]
    Device(#[from] DeviceError),

    #[error("Platform error: {0}")]
    Platform(#[from] PlatformError),

    #[error("Window monitoring error: {0}")]
    WindowMonitor(#[from] WindowMonitorError),

    #[error("Command execution failed: {command} - {reason}")]
    CommandExecution { command: String, reason: String },

    #[error("Validation error: {0}")]
    Validation(#[from] ValidationError),

    #[error("Retry operation failed after {attempts} attempts: {cause}")]
    RetryExhausted { attempts: u32, cause: String },

    #[error("Operation timed out after {timeout}")]
    Timeout { timeout: String },

    #[error("Circuit breaker opened after {failures} consecutive failures")]
    CircuitBreakerOpen { failures: u32 },

    #[error("Component degraded: {component} - {reason}")]
    Degradation { component: String, reason: String },

    #[error("Insufficient permissions for operation: {operation}")]
    InsufficientPermissions { operation: String, required: String },

    #[error("Mutex lock failed: {component}")]
    MutexLockFailed { component: String },

    #[error("Thread communication failed: {reason}")]
    ThreadCommunication { reason: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parsing error: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("JSON serialization error: {0}")]
    JsonSerialization(#[from] serde_json::Error),
}

impl QMKError {
    pub fn is_security_related(&self) -> bool {
        matches!(
            self,
            QMKError::CommandExecution { .. }
                | QMKError::Validation(ValidationError::InvalidPathSequence { .. })
                | QMKError::Validation(ValidationError::InvalidStringBytes { .. })
                | QMKError::InsufficientPermissions { .. }
        )
    }

    pub fn get_error_code(&self) -> u32 {
        match self {
            QMKError::Config(_) => 1000,
            QMKError::Device(_) => 2000,
            QMKError::Platform(_) => 3000,
            QMKError::WindowMonitor(_) => 4000,
            QMKError::CommandExecution { .. } => 5000,
            QMKError::Validation(_) => 6000,
            QMKError::RetryExhausted { .. } => 7000,
            QMKError::Timeout { .. } => 8000,
            QMKError::CircuitBreakerOpen { .. } => 9000,
            QMKError::Degradation { .. } => 9500,
            QMKError::InsufficientPermissions { .. } => 9600,
            QMKError::MutexLockFailed { .. } => 9700,
            QMKError::ThreadCommunication { .. } => 9800,
            QMKError::Io(_) => 9900,
            QMKError::TomlParse(_) => 10001,
            QMKError::JsonSerialization(_) => 10002,
        }
    }

    pub fn context_info(&self) -> ErrorContext {
        ErrorContext::new()
            .add("error_code", self.get_error_code().to_string())
            .add("security_related", self.is_security_related().to_string())
            .add("error_type", std::any::type_name::<QMKError>())
            .timestamp(std::time::SystemTime::now())
    }
}

/// Configuration-specific errors
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to parse configuration file: {file}")]
    ParseError { file: String },

    #[error("Configuration file not found: {path}")]
    NotFound { path: PathBuf },

    #[error("Invalid configuration value: {field} = {value}")]
    InvalidValue { field: String, value: String },

    #[error("Missing required configuration field: {field}")]
    MissingField { field: String },
}

/// Device communication errors
#[derive(Debug, Error)]
pub enum DeviceError {
    #[error("No QMK device found with VID:0x{vid:04X} PID:0x{pid:04X}")]
    DeviceNotFound { vid: u16, pid: u16 },

    #[error("Permission denied accessing device: {device}")]
    PermissionDenied { device: String },

    #[error("Device disconnected during operation")]
    Disconnected,

    #[error("Invalid device response: {reason}")]
    InvalidResponse { reason: String },

    #[error("Device communication timeout after {duration_ms}ms")]
    Timeout { duration_ms: u64 },
}

/// Platform-specific errors
#[derive(Debug, Error)]
pub enum PlatformError {
    #[error("Hyprland IPC connection failed: {reason}")]
    HyprlandConnection { reason: String },

    #[error("X11 display connection failed: {display}")]
    X11Connection { display: String },

    #[error("Windows service error: {code}")]
    WindowsService { code: u32 },

    #[error("macOS Core Foundation error: {code}")]
    MacOSCoreFoundation { code: i32 },

    #[error("Unsupported platform: {platform}")]
    UnsupportedPlatform { platform: String },
}

/// Window monitoring errors
#[derive(Debug, Error)]
pub enum WindowMonitorError {
    #[error("Failed to start window monitor: {reason}")]
    StartFailed { reason: String },

    #[error("Window monitor stopped unexpectedly: {reason}")]
    UnexpectedStop { reason: String },

    #[error("Invalid window information received: {reason}")]
    InvalidWindowInfo { reason: String },

    #[error("Window event listener error: {reason}")]
    EventListener { reason: String },
}

/// Input validation errors
#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Configuration field '{field}' validation failed: {reason}")]
    InvalidField { field: String, reason: String },

    #[error("Path '{path}' contains invalid sequence: {sequence}")]
    InvalidPathSequence { path: String, sequence: String },

    #[error("String '{value}' contains invalid bytes at position {pos}")]
    InvalidStringBytes { value: String, pos: usize },

    #[error("Numeric value {value} out of valid range [{min}, {max}] for field '{field}'")]
    OutOfRange { field: String, value: i64, min: i64, max: i64 },

    #[error("String exceeds maximum length of {max} characters")]
    StringTooLong { max: usize },

    #[error("Empty value not allowed for field '{field}'")]
    EmptyValue { field: String },
}

impl ValidationError {
    pub fn is_security_related(&self) -> bool {
        matches!(
            self,
            ValidationError::InvalidPathSequence { .. } | ValidationError::InvalidStringBytes { .. }
        )
    }
}

/// Command execution errors
#[derive(Debug, Error)]
pub enum CommandError {
    #[error("Invalid command arguments: {args}")]
    InvalidArgs { args: String },

    #[error("Security violation detected: {reason}")]
    SecurityViolation { reason: String },

    #[error("Command execution failed: {reason}")]
    ExecutionFailed { reason: String },

    #[error("Command output too large: {size} bytes")]
    OutputTooLarge { size: usize },

    #[error("Command path is not allowed: {path}")]
    InvalidPath { path: String },

    #[error("Command timed out after {timeout}")]
    Timeout { timeout: String },
}

/// Retry mechanism errors
#[derive(Debug, Error)]
pub enum RetryError<E: std::error::Error + Send + Sync + 'static> {
    #[error("Operation failed after {attempts} attempts")]
    Exhausted(E),

    #[error("Retry attempts exhausted: {cause}")]
    RetryExhausted { cause: E, attempts: u32 },

    #[error("Operation timed out after {timeout}")]
    Timeout { timeout: std::time::Duration },

    #[error("Operation failed: {cause}")]
    OperationFailed { cause: E },

    #[error("Circuit breaker opened: {failures} consecutive failures")]
    CircuitBreakerOpen { failures: u32 },
}

/// Error context for structured logging
#[derive(Debug, Clone)]
pub struct ErrorContext {
    data: std::collections::HashMap<String, String>,
}

impl ErrorContext {
    pub fn new() -> Self {
        Self {
            data: std::collections::HashMap::new(),
        }
    }

    pub fn add<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.data.insert(key.into(), value.into());
        self
    }

    pub fn timestamp(mut self, time: std::time::SystemTime) -> Self {
        if let Ok(duration) = time.duration_since(std::time::UNIX_EPOCH) {
            self.data.insert("timestamp".to_string(), duration.as_secs().to_string());
        }
        self
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.data.get(key).map(|s| s.as_str())
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.data.contains_key(key)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.data.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

impl Default for ErrorContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Result type alias for convenience
pub type QMKResult<T> = Result<T, QMKError>;

/// Result type alias for validation
pub type ValidationResult<T> = Result<T, ValidationError>;

/// Result type alias for command execution
pub type CommandResult<T> = Result<T, CommandError>;

/// Result type alias for retry operations
pub type RetryResult<T, E> = Result<T, RetryError<E>>;