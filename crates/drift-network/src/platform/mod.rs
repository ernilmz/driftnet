#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{ProxyLease, root_daemon};
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::ProxyLease;
#[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
pub struct ProxyLease;
#[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
impl ProxyLease {
    pub fn enable(_url: &str) -> std::io::Result<Self> {
        Err(std::io::Error::other(
            "System proxy integration is only available on macOS and Windows",
        ))
    }
}

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::ProxyLease;
