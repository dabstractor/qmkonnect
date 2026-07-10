use crate::core::{QMKError, QMKResult};
use tray_icon;
use std::path::Path;

fn load_icon(path: &std::path::Path) -> QMKResult<tray_icon::Icon> {
    let image = image::open(path).map_err(QMKError::Io)?.into_rgba8();
    let (width, height) = image.dimensions();
    let rgba = image.into_raw();
    tray_icon::Icon::from_rgba(rgba, width, height)
        .map_err(|e| QMKError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to create icon from RGBA: {}", e)
        ))
}

fn create_default_icon() -> tray_icon::Icon {
    let rgba = vec![255u8; 16 * 16 * 4];
    tray_icon::Icon::from_rgba(rgba, 16, 16)
        .map_err(|e| QMKError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to create default icon: {}", e)
        ))
}