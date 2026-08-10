use hidapi::HidError;
use std::fmt;

#[derive(Debug)]
pub enum QmkError {
    HidApiInitError(String),
    DeviceNotFound {
        vendor_id: Option<u16>,
        product_id: Option<u16>,
        usage_page: u16,
        usage: u16,
    },
    DeviceOpenError(String),
    InvalidHexValue(String),
    InvalidDecimalValue(String),
    SendReportError(HidError),
    HidReadError(String),
    NoResponseReceived(String),
    MissingRequiredParameter(String),
    RemovedFeature(String),
    PartialSendError {
        succeeded: usize,
        failed: usize,
    },
}

impl fmt::Display for QmkError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            QmkError::HidApiInitError(e) => write!(f, "Error initializing HID API: {}", e),
            QmkError::DeviceNotFound {
                vendor_id,
                product_id,
                usage_page,
                usage,
            } => {
                let vid = vendor_id
                    .map(|v| format!("0x{v:04X}"))
                    .unwrap_or_else(|| "any".into());
                let pid = product_id
                    .map(|p| format!("0x{p:04X}"))
                    .unwrap_or_else(|| "any".into());
                write!(
                    f,
                    "No device found with VID: {vid}, PID: {pid}, Usage Page: 0x{usage_page:04X}, Usage: 0x{usage:04X}"
                )
            }
            QmkError::DeviceOpenError(e) => write!(f, "Error opening device: {}", e),
            QmkError::InvalidHexValue(e) => write!(f, "Invalid hex value: {}", e),
            QmkError::InvalidDecimalValue(e) => write!(f, "Invalid decimal value: {}", e),
            QmkError::SendReportError(e) => write!(f, "Error sending report: {}", e),
            QmkError::HidReadError(e) => write!(f, "Error reading report: {}", e),
            QmkError::NoResponseReceived(e) => write!(f, "No response received: {}", e),
            QmkError::MissingRequiredParameter(param) => {
                write!(f, "Missing required parameter: {}", param)
            }
            QmkError::RemovedFeature(feature) => write!(f, "Feature removed: {}", feature),
            QmkError::PartialSendError { succeeded, failed } => {
                write!(
                    f,
                    "Message sent to {} devices, but failed for {} devices.",
                    succeeded, failed
                )
            }
        }
    }
}

impl std::error::Error for QmkError {}

impl From<HidError> for QmkError {
    fn from(error: HidError) -> Self {
        QmkError::SendReportError(error)
    }
}
