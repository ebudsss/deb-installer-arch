//! Riwayat konversi dan instalasi, disimpan sebagai JSON di
//! `~/.local/share/deb-installer/history.json`.

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallRecord {
    pub id: String,
    /// Nama paket Debian asli.
    pub deb_name: String,
    pub deb_version: String,
    pub deb_path: String,
    pub deb_sha256: String,
    /// Nama paket pacman hasil konversi.
    pub pkgname: String,
    pub pkgver: String,
    /// Lokasi `.pkg.tar.zst`.
    pub package_path: String,
    pub converted_at: i64,
    pub installed_at: Option<i64>,
    pub uninstalled_at: Option<i64>,
}

impl InstallRecord {
    pub fn is_installed(&self) -> bool {
        self.installed_at.is_some() && self.uninstalled_at.is_none()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct History {
    #[serde(default)]
    pub records: Vec<InstallRecord>,
}

impl History {
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| crate::Error::Other(format!("history.json rusak: {e}"))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    pub fn add(&mut self, record: InstallRecord) {
        self.records.retain(|r| r.pkgname != record.pkgname);
        self.records.push(record);
    }

    pub fn mark_installed(&mut self, pkgname: &str, at: i64) -> bool {
        if let Some(record) = self.records.iter_mut().find(|r| r.pkgname == pkgname) {
            record.installed_at = Some(at);
            record.uninstalled_at = None;
            true
        } else {
            false
        }
    }

    pub fn mark_uninstalled(&mut self, pkgname: &str, at: i64) -> bool {
        if let Some(record) = self.records.iter_mut().find(|r| r.pkgname == pkgname) {
            record.uninstalled_at = Some(at);
            true
        } else {
            false
        }
    }

    /// Menyelaraskan catatan "terpasang" dengan daftar paket yang benar-benar
    /// ada di sistem. Mengembalikan `true` bila ada yang berubah.
    ///
    /// Riwayat bisa basi bila paket dihapus di luar aplikasi (`pacman -R`,
    /// pembaruan sistem). Catatan yang sudah ditandai dihapus tidak diubah.
    pub fn reconcile(&mut self, installed: &std::collections::HashSet<String>, at: i64) -> bool {
        let mut changed = false;
        for record in &mut self.records {
            if record.installed_at.is_some()
                && record.uninstalled_at.is_none()
                && !installed.contains(&record.pkgname)
            {
                record.uninstalled_at = Some(at);
                changed = true;
            }
        }
        changed
    }

    pub fn find(&self, pkgname: &str) -> Option<&InstallRecord> {
        self.records.iter().find(|r| r.pkgname == pkgname)
    }

    /// Paket hasil konversi yang masih terpasang.
    pub fn installed(&self) -> impl Iterator<Item = &InstallRecord> {
        self.records.iter().filter(|r| r.is_installed())
    }
}

pub fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("DEB_INSTALLER_DATA_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".local/share/deb-installer")
}

pub fn default_history_path() -> PathBuf {
    data_dir().join("history.json")
}

pub fn default_cache_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("DEB_INSTALLER_CACHE_DIR") {
        return PathBuf::from(dir);
    }
    let base = std::env::var("XDG_CACHE_HOME").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        format!("{home}/.cache")
    });
    PathBuf::from(base).join("deb-installer")
}

pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// ID stabil untuk satu konversi.
pub fn record_id(deb_name: &str, sha256: &str) -> String {
    let short = sha256.chars().take(12).collect::<String>();
    format!("{deb_name}-{short}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(pkgname: &str) -> InstallRecord {
        InstallRecord {
            id: format!("id-{pkgname}"),
            deb_name: pkgname.trim_end_matches("-deb").to_string(),
            deb_version: "1.0-1".into(),
            deb_path: "/tmp/x.deb".into(),
            deb_sha256: "abc".into(),
            pkgname: pkgname.into(),
            pkgver: "1.0-1".into(),
            package_path: "/tmp/x.pkg.tar.zst".into(),
            converted_at: 100,
            installed_at: None,
            uninstalled_at: None,
        }
    }

    #[test]
    fn roundtrips_and_marks_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        let mut history = History::default();
        history.add(sample("app-deb"));
        history.mark_installed("app-deb", 200);
        history.save(&path).unwrap();

        let loaded = History::load(&path).unwrap();
        assert_eq!(loaded.records.len(), 1);
        assert!(loaded.find("app-deb").unwrap().is_installed());
        assert_eq!(loaded.installed().count(), 1);

        let mut loaded = loaded;
        assert!(loaded.mark_uninstalled("app-deb", 300));
        assert!(!loaded.find("app-deb").unwrap().is_installed());
        assert!(!loaded.mark_uninstalled("missing", 300));
    }

    #[test]
    fn missing_file_loads_empty_history() {
        let dir = tempfile::tempdir().unwrap();
        let history = History::load(&dir.path().join("nope.json")).unwrap();
        assert!(history.records.is_empty());
    }

    #[test]
    fn add_replaces_same_package() {
        let mut history = History::default();
        history.add(sample("app-deb"));
        let mut newer = sample("app-deb");
        newer.pkgver = "2.0-1".into();
        history.add(newer);
        assert_eq!(history.records.len(), 1);
        assert_eq!(history.records[0].pkgver, "2.0-1");
    }

    #[test]
    fn reconcile_marks_vanished_packages_as_removed() {
        use std::collections::HashSet;

        let mut history = History::default();
        history.add(sample("present-deb"));
        history.add(sample("vanished-deb"));
        history.mark_installed("present-deb", 100);
        history.mark_installed("vanished-deb", 100);

        let installed: HashSet<String> = ["present-deb".to_string()].into_iter().collect();
        assert!(history.reconcile(&installed, 500));

        assert!(history.find("present-deb").unwrap().is_installed());
        let vanished = history.find("vanished-deb").unwrap();
        assert!(!vanished.is_installed());
        assert_eq!(vanished.uninstalled_at, Some(500));

        // Tidak ada perubahan kedua kali.
        assert!(!history.reconcile(&installed, 600));
    }

    #[test]
    fn reconcile_keeps_converted_but_never_installed_records() {
        use std::collections::HashSet;

        let mut history = History::default();
        history.add(sample("converted-only-deb"));
        // Belum pernah dipasang: tidak boleh ditandai "dihapus".
        assert!(!history.reconcile(&HashSet::new(), 700));
        let record = history.find("converted-only-deb").unwrap();
        assert!(record.installed_at.is_none());
        assert!(record.uninstalled_at.is_none());
    }

    #[test]
    fn record_id_uses_hash_prefix() {
        assert_eq!(record_id("foo", "0123456789abcdef"), "foo-0123456789ab");
    }
}
