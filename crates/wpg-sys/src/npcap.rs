//! Npcap detection (Windows). On other operating systems [`detect`] reports "not installed".
//!
//! Npcap installs `wpcap.dll` / `Packet.dll` into `System32\Npcap` by default. Only in
//! "`WinPcap` API-compatible mode" are they also copied to `System32`, where a program
//! that links `wpcap.dll` finds them without adjusting the DLL search path.
//!
//! Windows builds with the `pcap` feature link `wpcap.dll` with `/DELAYLOAD` (see
//! `crates/wpg-cli/build.rs`), so `wpg.exe` starts without Npcap. Before the first libpcap
//! call the pcap backends call [`load`], which adds `System32\Npcap` to the DLL search path
//! (as Npcap recommends) and loads the DLL, so either install mode works and a missing
//! Npcap is an error message instead of a crash in the delay-load helper.

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
    /// The driver only lets administrators capture and send ("Restrict Npcap driver's
    /// access to Administrators only" at install time; the `AdminOnly` value of the
    /// `npcap` service parameters). `None` when it could not be read.
    pub admin_only: Option<bool>,
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

    /// `wpcap.dll` is present where [`load`] looks for it (`System32\Npcap` or `System32`).
    #[must_use]
    pub fn loadable(&self) -> bool {
        self.wpcap_dll.is_some() || self.winpcap_compatible
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
        admin_only: None,
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
    NpcapInfo {
        admin_only: admin_only(),
        ..detect_in(&Path::new(&root).join("System32"), registry_key_exists())
    }
}

#[cfg(windows)]
fn admin_only() -> Option<bool> {
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD,
    };
    let key: Vec<u16> = "SYSTEM\\CurrentControlSet\\Services\\npcap\\Parameters\0"
        .encode_utf16()
        .collect();
    let name: Vec<u16> = "AdminOnly\0".encode_utf16().collect();
    let mut value: u32 = 0;
    let mut size: u32 = 4;
    // SAFETY: `key` and `name` are NUL-terminated; `value` is a DWORD buffer of `size` (4) octets.
    let rc = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            std::ptr::from_mut(&mut value).cast(),
            &mut size,
        )
    };
    (rc == ERROR_SUCCESS).then_some(value != 0)
}

#[cfg(windows)]
fn registry_key_exists() -> bool {
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, HKEY, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY,
        KEY_WOW64_64KEY,
    };
    let path: Vec<u16> = "SOFTWARE\\Npcap\0".encode_utf16().collect();
    // The Npcap installer is a 32-bit program, so on 64-bit Windows its key lands in the
    // 32-bit view (`SOFTWARE\WOW6432Node\Npcap`); look at both views.
    [KEY_WOW64_64KEY, KEY_WOW64_32KEY].into_iter().any(|view| {
        let mut key: HKEY = std::ptr::null_mut();
        // SAFETY: `path` is NUL-terminated; `key` is a valid out-pointer and closed on success.
        unsafe {
            let rc = RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                path.as_ptr(),
                0,
                KEY_READ | view,
                &mut key,
            );
            if rc == ERROR_SUCCESS {
                RegCloseKey(key);
                true
            } else {
                false
            }
        }
    })
}

#[cfg(not(windows))]
fn detect_os() -> NpcapInfo {
    NpcapInfo::default()
}

/// Makes `wpcap.dll` loadable and loads it (Windows; a no-op returning `Ok` elsewhere).
///
/// Call it before the first libpcap function. The result is computed once per process.
///
/// # Errors
/// A message naming the problem and the Npcap download page when `wpcap.dll` cannot be
/// loaded (Npcap not installed, or a broken install).
pub fn load() -> Result<(), String> {
    load_os()
}

#[cfg(windows)]
fn load_os() -> Result<(), String> {
    use std::sync::OnceLock;
    static LOADED: OnceLock<Result<(), String>> = OnceLock::new();
    LOADED.get_or_init(load_wpcap).clone()
}

#[cfg(windows)]
fn load_wpcap() -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::System::LibraryLoader::{LoadLibraryW, SetDllDirectoryW};

    let wide = |s: &std::ffi::OsStr| -> Vec<u16> { s.encode_wide().chain([0]).collect() };
    let info = detect();
    if let Some(dir) = info.wpcap_dll.as_deref().and_then(Path::parent) {
        // SAFETY: NUL-terminated UTF-16 path.
        unsafe { SetDllDirectoryW(wide(dir.as_os_str()).as_ptr()) };
    }
    // SAFETY: NUL-terminated UTF-16 name. The module stays loaded for the process lifetime,
    // so the delay-load helper later resolves `wpcap.dll` to it.
    let module = unsafe { LoadLibraryW(wide("wpcap.dll".as_ref()).as_ptr()) };
    if module.is_null() {
        let err = std::io::Error::last_os_error();
        Err(if info.installed() {
            format!("cannot load wpcap.dll from Npcap ({err}); reinstall Npcap from https://npcap.com/#download and run `wpg doctor`")
        } else {
            "Npcap is not installed; install it from https://npcap.com/#download and run `wpg doctor`".to_owned()
        })
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
#[allow(clippy::unnecessary_wraps)] // same signature as the Windows version
fn load_os() -> Result<(), String> {
    Ok(())
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
        assert!(!i.winpcap_compatible && !i.loadable_without_path_tweak() && i.loadable());

        std::fs::write(root.join("wpcap.dll"), b"").unwrap();
        assert!(detect_in(&root, true).loadable_without_path_tweak());
    }

    #[cfg(not(windows))]
    #[test]
    fn not_installed_elsewhere() {
        assert!(!detect().installed());
        assert_eq!(load(), Ok(()));
    }
}
