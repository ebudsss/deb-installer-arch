//! Menemukan cara membuka aplikasi dari paket yang terpasang.
//!
//! Urutannya: berkas `.desktop` milik paket (cara yang benar untuk aplikasi
//! GUI), lalu binari di `/usr/bin` sebagai cadangan untuk paket tanpa entri
//! menu.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::{Command, Stdio};

/// Jenis peluncur yang ditemukan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LaunchKind {
    /// Berkas `.desktop` di `/usr/share/applications`.
    Desktop,
    /// Binari yang dipanggil langsung.
    Binary,
}

/// Cara membuka sebuah paket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchTarget {
    pub kind: LaunchKind,
    /// Path absolut berkas `.desktop` atau binari.
    pub value: String,
    /// Nama yang ditampilkan di UI.
    pub label: String,
}

/// Berkas yang dimiliki paket menurut `pacman -Ql`.
///
/// Format keluarannya `nama-paket /path/berkas`, satu per baris. Kegagalan
/// (paket tidak terpasang, pacman tidak ada) menghasilkan daftar kosong.
pub fn package_files(pkgname: &str) -> Vec<String> {
    if pkgname.trim().is_empty() {
        return Vec::new();
    }
    let output = Command::new("pacman").args(["-Ql", pkgname]).output();
    match output {
        Ok(out) if out.status.success() => {
            parse_package_files(&String::from_utf8_lossy(&out.stdout))
        }
        _ => Vec::new(),
    }
}

/// Memisahkan kolom path pada keluaran `pacman -Ql`.
fn parse_package_files(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|line| {
            line.split_once(' ')
                .map(|(_, path)| path.trim().to_string())
        })
        .filter(|path| path.starts_with('/') && !path.ends_with('/'))
        .collect()
}

/// Mencari peluncur untuk paket yang terpasang.
pub fn discover(pkgname: &str) -> Option<LaunchTarget> {
    discover_from_files(pkgname, &package_files(pkgname))
}

/// Bagian murni dari [`discover`], agar bisa diuji tanpa pacman.
pub fn discover_from_files(pkgname: &str, files: &[String]) -> Option<LaunchTarget> {
    // Entri menu: pilih yang namanya paling sederhana agar hasilnya stabil.
    let desktop = files
        .iter()
        .filter(|path| path.starts_with("/usr/share/applications/") && path.ends_with(".desktop"))
        .min_by_key(|path| path.len());

    match desktop {
        Some(path) => {
            let label = desktop_name(Path::new(path)).unwrap_or_else(|| display_name(pkgname));
            Some(LaunchTarget {
                kind: LaunchKind::Desktop,
                value: path.clone(),
                label,
            })
        }
        None => binary_target(pkgname, files),
    }
}

/// Cadangan untuk paket tanpa entri menu.
fn binary_target(pkgname: &str, files: &[String]) -> Option<LaunchTarget> {
    let mut bins: Vec<&String> = files
        .iter()
        .filter(|path| path.starts_with("/usr/bin/") && !path.contains("/../"))
        .collect();
    // Utamakan binari asli; skrip shell tetap dipakai bila hanya itu yang ada.
    bins.sort_by_key(|path| path.ends_with(".sh"));
    let bin = bins.first()?;
    Some(LaunchTarget {
        kind: LaunchKind::Binary,
        value: (*bin).clone(),
        label: Path::new(bin)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| display_name(pkgname)),
    })
}

/// Membaca `Name=` dari berkas `.desktop`.
fn desktop_name(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    text.lines()
        .find_map(|line| line.strip_prefix("Name="))
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
}

/// Nama tampilan dari nama paket: `ghostex-deb` → `Ghostex`.
fn display_name(pkgname: &str) -> String {
    let base = pkgname.trim_end_matches("-deb").replace(['-', '_'], " ");
    let mut chars = base.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => pkgname.to_string(),
    }
}

/// Membuka aplikasi. Proses dijalankan terpisah dari aplikasi ini, sehingga
/// menutup DebInstaller tidak menutup aplikasi yang baru dibuka.
pub fn launch(target: &LaunchTarget) -> Result<()> {
    let path = Path::new(&target.value);
    if !path.exists() {
        return Err(Error::Install(format!(
            "berkas peluncur tidak ditemukan: {}",
            target.value
        )));
    }
    match target.kind {
        LaunchKind::Desktop => {
            let id = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().to_string())
                .ok_or_else(|| Error::Install("nama berkas .desktop tidak valid".into()))?;
            spawn_detached("gtk-launch", &[id.as_str()])
                .or_else(|_| spawn_detached("gio", &["launch", target.value.as_str()]))
        }
        LaunchKind::Binary => spawn_detached(&target.value, &[]),
    }
}

fn spawn_detached(program: &str, args: &[&str]) -> Result<()> {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| Error::Install(format!("tidak bisa menjalankan `{program}`: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_pacman_file_list() {
        let out = "pkg /usr/bin/app\npkg /usr/share/\npkg /usr/share/applications/app.desktop\n";
        let files = parse_package_files(out);
        // Entri direktori dibuang; discover hanya butuh berkas.
        assert_eq!(
            files,
            paths(&["/usr/bin/app", "/usr/share/applications/app.desktop"])
        );
    }

    #[test]
    fn prefers_desktop_entry() {
        let files = paths(&[
            "/usr/bin/ghostex",
            "/usr/bin/gx",
            "/usr/share/applications/ghostex.desktop",
        ]);
        let target = discover_from_files("ghostex-deb", &files).unwrap();
        assert_eq!(target.kind, LaunchKind::Desktop);
        assert_eq!(target.value, "/usr/share/applications/ghostex.desktop");
        // Berkas .desktop tidak ada di disk saat uji, jadi nama paket dipakai.
        assert_eq!(target.label, "Ghostex");
    }

    #[test]
    fn falls_back_to_binary() {
        let files = paths(&["/usr/bin/run.sh", "/usr/bin/app", "/opt/app/data"]);
        let target = discover_from_files("app-deb", &files).unwrap();
        assert_eq!(target.kind, LaunchKind::Binary);
        assert_eq!(target.value, "/usr/bin/app");
        assert_eq!(target.label, "app");
    }

    #[test]
    fn reports_nothing_when_package_has_no_executable() {
        let files = paths(&["/usr/share/doc/app/README", "/opt/app/data"]);
        assert_eq!(discover_from_files("app-deb", &files), None);
        assert_eq!(discover_from_files("app-deb", &[]), None);
    }
}
