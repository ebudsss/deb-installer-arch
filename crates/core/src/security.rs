//! Pemeriksaan keamanan path: menolak entri yang bisa merusak sistem atau
//! membajak paket inti.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sensitivity {
    /// Menolak konversi: path milik pacman/database sistem.
    Forbidden,
    /// Izinkan dengan peringatan keras.
    Warning,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensitivePath {
    pub path: String,
    pub reason: String,
    pub level: Sensitivity,
}

const FORBIDDEN_PREFIXES: &[(&str, &str)] = &[
    ("/etc/pacman.conf", "konfigurasi pacman"),
    ("/etc/pacman.d/", "konfigurasi repositori pacman"),
    ("/var/lib/pacman/", "database pacman"),
    ("/usr/bin/pacman", "binari pacman"),
    ("/usr/bin/pacman-conf", "binari pacman"),
    ("/usr/bin/makepkg", "binari makepkg"),
    ("/usr/lib/libalpm.so", "library libalpm"),
    ("/usr/lib/libalpm", "library libalpm"),
    ("/var/lib/dpkg/", "database dpkg (tidak berlaku di Arch)"),
    ("/var/lib/apt/", "database apt (tidak berlaku di Arch)"),
    ("/etc/apt/", "konfigurasi apt (tidak berlaku di Arch)"),
];

const WARNING_PREFIXES: &[(&str, &str)] = &[
    ("/boot/", "berkas bootloader/kernel"),
    ("/etc/fstab", "konfigurasi mount sistem"),
    ("/etc/passwd", "database pengguna"),
    ("/etc/shadow", "database kata sandi"),
    ("/etc/sudoers", "konfigurasi sudo"),
    ("/etc/ld.so.conf", "konfigurasi dynamic linker"),
    ("/usr/lib/systemd/system/", "unit systemd sistem"),
    ("/usr/lib/udev/rules.d/", "aturan udev"),
    ("/etc/systemd/system/", "unit systemd"),
    ("/usr/share/glib-2.0/schemas/", "skema GSettings bersama"),
];

/// Menilai satu path absolut (setelah normalisasi) terhadap daftar path
/// sensitif.
fn matches(path: &str, prefix: &str) -> bool {
    if prefix.ends_with('/') {
        path.starts_with(prefix)
    } else {
        path == prefix || path.starts_with(&format!("{prefix}/"))
    }
}

pub fn check_path(path: &str) -> Option<SensitivePath> {
    for (prefix, reason) in FORBIDDEN_PREFIXES {
        if matches(path, prefix) {
            return Some(SensitivePath {
                path: path.to_string(),
                reason: reason.to_string(),
                level: Sensitivity::Forbidden,
            });
        }
    }
    for (prefix, reason) in WARNING_PREFIXES {
        if matches(path, prefix) {
            return Some(SensitivePath {
                path: path.to_string(),
                reason: reason.to_string(),
                level: Sensitivity::Warning,
            });
        }
    }
    None
}

/// Memeriksa seluruh daftar path; mengembalikan yang sensitif saja.
pub fn scan(paths: impl IntoIterator<Item = String>) -> Vec<SensitivePath> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for path in paths {
        if let Some(hit) = check_path(&path) {
            if seen.insert(hit.path.clone()) {
                out.push(hit);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_pacman_database_as_forbidden() {
        let hit = check_path("/var/lib/pacman/local/foo").unwrap();
        assert_eq!(hit.level, Sensitivity::Forbidden);
        assert!(check_path("/usr/bin/pacman").is_some());
    }

    #[test]
    fn flags_boot_and_system_units_as_warning() {
        assert_eq!(
            check_path("/boot/vmlinuz-linux").unwrap().level,
            Sensitivity::Warning
        );
        assert_eq!(
            check_path("/usr/lib/systemd/system/foo.service")
                .unwrap()
                .level,
            Sensitivity::Warning
        );
    }

    #[test]
    fn allows_regular_paths() {
        assert!(check_path("/usr/bin/app").is_none());
        assert!(check_path("/etc/pacman.conf.bak").is_none());
    }

    #[test]
    fn scan_deduplicates() {
        let hits = scan(vec![
            "/boot/a".into(),
            "/boot/a".into(),
            "/usr/bin/app".into(),
        ]);
        assert_eq!(hits.len(), 1);
    }
}
