//! Presence of the libpcap shared library on Unix (no dependency on libpcap itself).

use std::path::{Path, PathBuf};

/// Whether a file name looks like the libpcap runtime (`libpcap.so.1.10.4`, `libpcap.A.dylib`).
#[must_use]
pub fn is_libpcap_name(name: &str) -> bool {
    name.starts_with("libpcap.so") || name == "libpcap.dylib" || name == "libpcap.A.dylib"
}

/// Searches `dirs` (non-recursively) for a libpcap runtime.
#[must_use]
pub fn find_in(dirs: &[PathBuf]) -> Option<PathBuf> {
    for d in dirs {
        let Ok(rd) = std::fs::read_dir(d) else {
            continue;
        };
        let mut names: Vec<_> = rd
            .flatten()
            .filter(|e| is_libpcap_name(&e.file_name().to_string_lossy()))
            .map(|e| e.path())
            .collect();
        names.sort();
        if let Some(p) = names.into_iter().next() {
            return Some(p);
        }
    }
    None
}

fn standard_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = [
        "/usr/lib",
        "/usr/lib64",
        "/lib",
        "/lib64",
        "/usr/local/lib",
        "/opt/homebrew/lib",
    ]
    .iter()
    .map(PathBuf::from)
    .collect();
    // Debian / Ubuntu multiarch: /usr/lib/x86_64-linux-gnu, /lib/aarch64-linux-gnu, …
    for base in ["/usr/lib", "/lib"] {
        if let Ok(rd) = std::fs::read_dir(base) {
            for e in rd.flatten() {
                if e.file_name().to_string_lossy().contains("-linux-") {
                    dirs.push(e.path());
                }
            }
        }
    }
    dirs
}

/// Locates libpcap. On macOS the library ships with the OS (inside the dyld shared cache
/// on recent releases, so the file may not exist on disk) and is always reported present.
/// Windows uses [`crate::npcap`] instead and returns `None` here.
#[must_use]
pub fn find() -> Option<PathBuf> {
    if cfg!(windows) {
        None
    } else if cfg!(target_os = "macos") {
        Some(PathBuf::from("/usr/lib/libpcap.A.dylib"))
    } else {
        find_in(&standard_dirs())
    }
}

/// Whether the development symlink (`libpcap.so`) exists, i.e. `--features pcap` can link.
#[must_use]
pub fn has_dev_symlink(dir: &Path) -> bool {
    dir.join("libpcap.so").exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert!(is_libpcap_name("libpcap.so.1.10.4"));
        assert!(is_libpcap_name("libpcap.so"));
        assert!(is_libpcap_name("libpcap.A.dylib"));
        assert!(!is_libpcap_name("libpcap-dev.txt"));
        assert!(!is_libpcap_name("libpng.so.16"));
    }

    #[test]
    fn find_in_fixture() {
        let d = crate::testutil::tempdir("pcaplib");
        assert_eq!(find_in(std::slice::from_ref(&d)), None);
        std::fs::write(d.join("libpcap.so.1.10.4"), b"").unwrap();
        assert_eq!(
            find_in(std::slice::from_ref(&d)),
            Some(d.join("libpcap.so.1.10.4"))
        );
        assert!(!has_dev_symlink(&d));
    }
}
