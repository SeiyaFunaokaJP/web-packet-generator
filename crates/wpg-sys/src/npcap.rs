//! Npcap detection (Windows). On other operating systems [`detect`] reports "not installed".
//!
//! Npcap installs `wpcap.dll` / `Packet.dll` into `System32\Npcap` by default. Only in
//! "`WinPcap` API-compatible mode" are they also copied to `System32`, where a program
//! that links `wpcap.dll` finds them without adjusting the DLL search path.

use std::path::{Path, PathBuf};

use serde::Serialize;

/// What was found.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct NpcapInfo {
    /// `HKLM\SOFTWARE\Npcap` exists.
    pub registry_key: bool,
    /// `wpcap.dll` inside `System32\Npcap`.
    pub wpcap_dll: Option<PathBuf>,
    /// `Packet.dll` inside `System32\Npcap`.
    pub packet_dll: Option<PathBuf>,
    /// `wpcap.dll` directly in `System32` (WinPcap-compatible install).
    pub winpcap_compatible: bool,
}

impl NpcapInfo {
    /// Some trace of Npcap is present.
    #[must_use]
    pub fn installed(&self) -> bool {
        self.registry_key
            || self.wpcap_dll.is_some()
            || self.packet_dll.is_some()
            || self.winpcap_compatible
    }

    /// `wpcap.dll` can be loaded by a plain `LoadLibrary("wpcap.dll")`.
    #[must_use]
    pub fn loadable_without_path_tweak(&self) -> bool {
        self.winpcap_compatible
    }
}

/// Looks at the file system below `system32` (pure; tested with fixtures).
#[must_use]
pub fn detect_in(system32: &Path, registry_key: bool) -> NpcapInfo {
    let dir = system32.join("Npcap");
    let existing = |p: PathBuf| p.is_file().then_some(p);
    NpcapInfo {
        registry_key,
        wpcap_dll: existing(dir.join("wpcap.dll")),
        packet_dll: existing(dir.join("Packet.dll")),
        winpcap_compatible: system32.join("wpcap.dll").is_file(),
    }
}

/// Detects Npcap on this machine.
#[must_use]
pub fn detect() -> NpcapInfo {
    detect_os()
}

#[cfg(windows)]
fn detect_os() -> NpcapInfo {
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
    detect_in(&Path::new(&root).join("System32"), registry_key_exists())
}

#[cfg(windows)]
fn registry_key_exists() -> bool {
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, HKEY, HKEY_LOCAL_MACHINE, KEY_READ,
    };
    let path: Vec<u16> = "SOFTWARE\\Npcap\0".encode_utf16().collect();
    let mut key: HKEY = std::ptr::null_mut();
    // SAFETY: `path` is NUL-terminated; `key` is a valid out-pointer and closed on success.
    unsafe {
        let rc = RegOpenKeyExW(HKEY_LOCAL_MACHINE, path.as_ptr(), 0, KEY_READ, &mut key);
        if rc == ERROR_SUCCESS {
            RegCloseKey(key);
            true
        } else {
            false
        }
    }
}

#[cfg(not(windows))]
fn detect_os() -> NpcapInfo {
    NpcapInfo::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_layouts() {
        let root = crate::testutil::tempdir("npcap");
        assert!(!detect_in(&root, false).installed());
        assert!(detect_in(&root, true).installed());

        std::fs::create_dir_all(root.join("Npcap")).unwrap();
        std::fs::write(root.join("Npcap/wpcap.dll"), b"").unwrap();
        std::fs::write(root.join("Npcap/Packet.dll"), b"").unwrap();
        let i = detect_in(&root, true);
        assert!(i.installed() && i.wpcap_dll.is_some() && i.packet_dll.is_some());
        assert!(!i.winpcap_compatible && !i.loadable_without_path_tweak());

        std::fs::write(root.join("wpcap.dll"), b"").unwrap();
        assert!(detect_in(&root, true).loadable_without_path_tweak());
    }

    #[cfg(not(windows))]
    #[test]
    fn not_installed_elsewhere() {
        assert!(!detect().installed());
    }
}
