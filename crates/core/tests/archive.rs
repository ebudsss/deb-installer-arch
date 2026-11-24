//! Uji integrasi untuk dukungan arsip tar: deteksi tata letak, konversi, dan
//! penolakan path berbahaya.

use deb_installer_core::archive::{ArchiveLayout, ArchivePackage};
use deb_installer_core::pkgbuild::BuildOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

struct TarBuilder {
    entries: Vec<(String, Vec<u8>, u32, Option<String>)>,
}

impl TarBuilder {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    fn dir(mut self, path: &str) -> Self {
        self.entries
            .push((path.to_string(), Vec::new(), 0o755, None));
        self
    }

    fn file(mut self, path: &str, content: &[u8], mode: u32) -> Self {
        self.entries
            .push((path.to_string(), content.to_vec(), mode, None));
        self
    }

    fn symlink(mut self, path: &str, target: &str) -> Self {
        self.entries.push((
            path.to_string(),
            Vec::new(),
            0o777,
            Some(target.to_string()),
        ));
        self
    }

    fn write_gz(self, dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        let mut tar_bytes = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut tar_bytes);
            for (entry_path, content, mode, link) in &self.entries {
                let mut header = tar::Header::new_gnu();
                header.set_mtime(1_700_000_000);
                if let Some(target) = link {
                    header.set_entry_type(tar::EntryType::Symlink);
                    header.set_size(0);
                    header.set_mode(*mode);
                    header.set_link_name(target).unwrap();
                    header.set_cksum();
                    builder
                        .append_data(&mut header, format!("./{entry_path}"), std::io::empty())
                        .unwrap();
                } else if content.is_empty() && entry_path.ends_with('/') {
                    header.set_entry_type(tar::EntryType::Directory);
                    header.set_size(0);
                    header.set_mode(*mode);
                    header.set_cksum();
                    builder
                        .append_data(&mut header, format!("./{entry_path}"), std::io::empty())
                        .unwrap();
                } else {
                    header.set_size(content.len() as u64);
                    header.set_mode(*mode);
                    header.set_cksum();
                    builder
                        .append_data(&mut header, format!("./{entry_path}"), content.as_slice())
                        .unwrap();
                }
            }
            builder.finish().unwrap();
        }
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        enc.write_all(&tar_bytes).unwrap();
        let gz = enc.finish().unwrap();
        std::fs::write(&path, gz).unwrap();
        path
    }
}

#[test]
fn installs_portable_archive_into_opt_with_bin_links() {
    let dir = tempfile::tempdir().unwrap();
    let archive = TarBuilder::new()
        .dir("omniget/")
        .file("omniget/bin/omniget", b"#!/bin/sh\necho hi\n", 0o755)
        .file("omniget/bin/omniget-hook", b"#!/bin/sh\n", 0o755)
        .file("omniget/lib/libx.so", b"\x7fELF", 0o644)
        .file("omniget/README.md", b"readme", 0o644)
        .file(
            "omniget/omniget.desktop",
            b"[Desktop Entry]\nType=Application\nName=OmniGet\nComment=Downloads things\nExec=omniget\n",
            0o644,
        )
        .write_gz(dir.path(), "omniget-1.2.3-linux-x64.tar.gz");

    let pkg = ArchivePackage::open(&archive).unwrap();
    assert_eq!(pkg.name, "omniget");
    assert_eq!(pkg.version, "1.2.3");
    assert_eq!(pkg.suggested_pkgname(), "omniget-deb");
    assert_eq!(
        pkg.layout,
        ArchiveLayout::Portable {
            root: Some("omniget".to_string())
        }
    );
    assert_eq!(
        pkg.executables,
        vec!["omniget".to_string(), "omniget-hook".to_string()]
    );
    assert_eq!(pkg.description.as_deref(), Some("Downloads things"));

    let out = dir.path().join("out");
    let staging = dir.path().join("stage");
    let result = pkg.build(&BuildOptions::new(&staging, &out)).unwrap();

    assert_eq!(result.pkgname, "omniget-deb");
    assert_eq!(result.pkgver, "1.2.3");
    assert!(result.package_path.exists());
    assert!(result
        .notes
        .iter()
        .any(|n| n.contains("portable") && n.contains("/opt/omniget-deb")));

    // Isi ditaruh di bawah /opt, root arsip dibuang.
    assert!(staging.join("opt/omniget-deb/bin/omniget").exists());
    assert!(staging.join("opt/omniget-deb/lib/libx.so").exists());
    assert!(!staging.join("omniget").exists());

    // Binari ditautkan ke /usr/bin, menunjuk path absolut di /opt.
    let link = std::fs::read_link(staging.join("usr/bin/omniget")).unwrap();
    assert_eq!(link.to_string_lossy(), "/opt/omniget-deb/bin/omniget");
    let hook = std::fs::read_link(staging.join("usr/bin/omniget-hook")).unwrap();
    assert_eq!(hook.to_string_lossy(), "/opt/omniget-deb/bin/omniget-hook");

    // .desktop dipindahkan ke share/applications agar terdaftar.
    assert!(staging
        .join("usr/share/applications/omniget.desktop")
        .exists());

    // Metadata paket memakai asal arsip, bukan Debian.
    let pkginfo = std::fs::read_to_string(staging.join(".PKGINFO")).unwrap();
    assert!(pkginfo.contains("pkgname = omniget-deb"));
    assert!(pkginfo.contains("xdata = archive-layout=portable"));
    assert!(pkginfo.contains("archive-sha256="));
    assert!(!pkginfo.contains("debian-"));
}

#[test]
fn keeps_system_layout_archives_as_is() {
    let dir = tempfile::tempdir().unwrap();
    let archive = TarBuilder::new()
        .file("usr/bin/tool", b"#!/bin/sh\n", 0o755)
        .file("usr/share/doc/tool/README", b"docs", 0o644)
        .write_gz(dir.path(), "tool-2.0.0.tar.gz");

    let pkg = ArchivePackage::open(&archive).unwrap();
    assert_eq!(pkg.layout, ArchiveLayout::System);
    assert!(pkg.executables.is_empty());

    let out = dir.path().join("out");
    let staging = dir.path().join("stage");
    pkg.build(&BuildOptions::new(&staging, &out)).unwrap();

    assert!(staging.join("usr/bin/tool").exists());
    assert!(staging.join("usr/share/doc/tool/README").exists());
    // Tidak ada penambahan tautan untuk tata letak sistem.
    assert!(!staging.join("opt").exists());
}

#[test]
fn flat_archives_are_placed_under_opt() {
    let dir = tempfile::tempdir().unwrap();
    let archive = TarBuilder::new()
        .file("mytool", b"#!/bin/sh\n", 0o755)
        .file("LICENSE", b"MIT", 0o644)
        .write_gz(dir.path(), "mytool.tar.gz");

    let pkg = ArchivePackage::open(&archive).unwrap();
    assert_eq!(pkg.layout, ArchiveLayout::Portable { root: None });
    assert_eq!(pkg.version, "1.0.0");

    let out = dir.path().join("out");
    let staging = dir.path().join("stage");
    pkg.build(&BuildOptions::new(&staging, &out)).unwrap();

    assert!(staging.join("opt/mytool-deb/mytool").exists());
    assert!(staging.join("opt/mytool-deb/LICENSE").exists());
    let link = std::fs::read_link(staging.join("usr/bin/mytool")).unwrap();
    assert_eq!(link.to_string_lossy(), "/opt/mytool-deb/mytool");
}

#[test]
fn refuses_archives_touching_protected_paths() {
    let dir = tempfile::tempdir().unwrap();
    let archive = TarBuilder::new()
        .file("usr/bin/tool", b"#!/bin/sh\n", 0o755)
        .file("var/lib/pacman/local/evil/desc", b"x", 0o644)
        .write_gz(dir.path(), "evil-1.0.0.tar.gz");

    let pkg = ArchivePackage::open(&archive).unwrap();
    assert_eq!(pkg.layout, ArchiveLayout::System);

    let out = dir.path().join("out");
    let staging = dir.path().join("stage");
    let err = pkg.build(&BuildOptions::new(&staging, &out)).unwrap_err();
    assert!(err.to_string().contains("dilindungi"), "{err}");
    assert!(!staging.join("var/lib/pacman").exists());
}

#[test]
fn rejects_unsupported_archive_types() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("thing.zip");
    std::fs::write(&path, b"PK\x03\x04").unwrap();
    let err = ArchivePackage::open(&path).unwrap_err();
    assert!(err.to_string().contains("bukan arsip tar"), "{err}");
}

#[test]
fn symlinks_inside_archive_survive() {
    let dir = tempfile::tempdir().unwrap();
    let archive = TarBuilder::new()
        .dir("app/")
        .file("app/bin/real", b"#!/bin/sh\n", 0o755)
        .symlink("app/bin/alias", "real")
        .write_gz(dir.path(), "app-0.1.0.tar.gz");

    let pkg = ArchivePackage::open(&archive).unwrap();
    let out = dir.path().join("out");
    let staging = dir.path().join("stage");
    pkg.build(&BuildOptions::new(&staging, &out)).unwrap();

    let link = std::fs::read_link(staging.join("opt/app-deb/bin/alias")).unwrap();
    assert_eq!(link.to_string_lossy(), "real");
}
