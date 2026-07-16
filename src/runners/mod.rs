#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub mod linux;

use std::error::Error;

// Platform-specific runner trait
pub trait PlatformRunner {
    fn run(&mut self, args: &[String]) -> Result<(), Box<dyn Error>>;
}

// Create platform-specific runner
pub fn create_runner(verbose: bool) -> Result<Box<dyn PlatformRunner>, Box<dyn Error>> {
    #[cfg(target_os = "windows")]
    {
        Ok(Box::new(windows::WindowsRunner::new(verbose)))
    }

    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(macos::MacOSRunner::new(verbose)))
    }

    #[cfg(target_os = "linux")]
    {
        Ok(Box::new(linux::LinuxRunner::new(verbose)))
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Err("Unsupported platform".into())
    }
}
