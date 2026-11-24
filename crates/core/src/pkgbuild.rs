//! Pembangun paket pacman dari isi `.deb`.
//!
//! Alur: ekstrak `data.tar` ke direktori staging dengan normalisasi path →
//! tulis `.PKGINFO`, `.INSTALL`, `.MTREE` → kemas ulang jadi
//! `.pkg.tar.zst`.

use crate::control::Control;
use crate::deb::DebPackage;
use crate::deps::{DepResolution, DepStatus};
use crate::error::{Error, Result};
use crate::mtree::{self, MtreeEntry, MtreeKind};
use crate::normalize;
use crate::security::{self, SensitivePath};
use crate::scripts::{self, ScriptReport};
use md5::Md5;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Nama paket hasil konversi selalu diberi akhiran ini agar tidak bentrok
/// dengan paket repo resmi.
pub const PACKAGE_SUFFIX: &str = "-deb";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildOptions {
    /// Direktori kerja untuk ekstraksi; dibuat bila belum ada.
    pub staging_dir: PathBuf,
    /// Direktori keluaran `.pkg.tar.zst`.
    pub output_dir: PathBuf,
    /// Timpa nama paket (default `<nama deb>-deb`).
    pub package_name: Option<String>,
    /// Sertakan `.INSTALL` hasil terjemahan script maintainer.
    pub include_install_script: bool,
    /// Level kompresi zstd (1-22). Level tinggi hampir tidak sepadan untuk
    /// paket besar: pada 295 MB payload, level 10 ≈ 2 detik (118 MB) sedangkan
    /// level 19 ≈ 20 detik (107 MB).
    pub compression_level: i32,
    /// Jumlah worker thread zstd. `0` = otomatis (maks. 8).
    pub compression_threads: u32,
    /// Identitas pembuat paket untuk `.PKGINFO`.
    pub packager: String,
}

impl BuildOptions {
    pub fn new(staging_dir: impl Into<PathBuf>, output_dir: impl Into<PathBuf>) -> Self {
        Self {
            staging_dir: staging_dir.into(),
            output_dir: output_dir.into(),
            package_name: None,
            include_install_script: true,
            compression_level: 10,
            compression_threads: 0,
            packager: "DebInstaller <noreply@localhost>".to_string(),
        }
    }
}

/// Kemajuan konversi, dilaporkan agar UI tidak terlihat menggantung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildStage {
    /// Jumlah berkas yang sudah diekstrak.
    Extracting { files: usize },
    /// Byte tar yang sudah dikompresi dari total perkiraan.
    Packing { written: u64, total: u64 },
}

impl BuildStage {
    /// Pesan singkat berbahasa Indonesia untuk log.
    pub fn message(&self) -> String {
        match self {
            BuildStage::Extracting { files } => format!("Mengekstrak berkas ({files} berkas)"),
            BuildStage::Packing { written, total } => {
                let percent = if *total == 0 {
                    100
                } else {
                    ((*written as f64 / *total as f64) * 100.0).min(100.0) as u32
                };
                format!("Mengompresi paket ({percent}%)")
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildResult {
    pub package_path: PathBuf,
    pub pkgname: String,
    pub pkgver: String,
    pub pkgrel: String,
    pub arch: String,
    pub installed_size: u64,
    pub file_count: usize,
    /// Path sensitif yang ditemukan (peringatan).
    pub warnings: Vec<SensitivePath>,
    /// Catatan non-fatal selama konversi.
    pub notes: Vec<String>,
    /// Isi `.INSTALL` bila dihasilkan.
    pub install_script: Option<String>,
    /// Nama paket dependensi Arch yang ditulis ke `.PKGINFO`.
    pub dependencies: Vec<String>,
}

impl BuildResult {
    /// Nama versi lengkap gaya pacman.
    pub fn full_version(&self) -> String {
        format!("{}-{}", self.pkgver, self.pkgrel)
    }
}

/// Versi Debian yang sudah diterjemahkan ke skema Arch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchVersion {
    pub epoch: Option<u32>,
    pub pkgver: String,
    pub pkgrel: String,
}

impl ArchVersion {
    pub fn full(&self) -> String {
        match self.epoch {
            Some(e) => format!("{e}:{}-{}", self.pkgver, self.pkgrel),
            None => format!("{}-{}", self.pkgver, self.pkgrel),
        }
    }
}

/// `[epoch:]upstream[-revision]` → `(epoch, pkgver, pkgrel)`.
pub fn convert_version(debian_version: &str) -> ArchVersion {
    let version = debian_version.trim();

    let (epoch, rest) = match version.split_once(':') {
        Some((head, tail)) if head.chars().all(|c| c.is_ascii_digit()) && !head.is_empty() => {
            (head.parse::<u32>().ok(), tail)
        }
        _ => (None, version),
    };

    let (upstream, revision) = match rest.rsplit_once('-') {
        Some((u, r)) => (u, Some(r)),
        None => (rest, None),
    };

    let pkgver = sanitize_pkgver(upstream);
    let pkgrel = revision
        .map(sanitize_pkgrel)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "1".to_string());

    ArchVersion {
        epoch,
        pkgver,
        pkgrel,
    }
}

/// Karakter yang sah di `pkgver` Arch; sisanya diganti titik.
fn sanitize_pkgver(input: &str) -> String {
    let mut out: String = input
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '~') {
                c
            } else {
                '.'
            }
        })
        .collect();
    if out.is_empty() {
        out.push('0');
    }
    out
}

/// `pkgrel` harus berupa angka dan titik.
fn sanitize_pkgrel(input: &str) -> String {
    let mut out: String = input
        .chars()
        .map(|c| if c.is_ascii_digit() || c == '.' { c } else { '.' })
        .collect();
    while out.contains("..") {
        out = out.replace("..", ".");
    }
    out = out.trim_matches('.').to_string();
    if out.is_empty() {
        "1".to_string()
    } else if !out.starts_with(|c: char| c.is_ascii_digit()) {
        format!("1.{out}")
    } else {
        out
    }
}

/// Arsitektur Debian → Arch.
pub fn convert_arch(debian_arch: &str) -> String {
    match debian_arch.trim() {
        "amd64" => "x86_64",
        "i386" | "i686" => "i686",
        "arm64" => "aarch64",
        "armhf" | "armel" => "armv7h",
        "all" | "any" => "any",
        other => other,
    }
    .to_string()
}

/// Nama paket Arch: sanitasi + akhiran `-deb`.
pub fn arch_package_name(debian_name: &str) -> String {
    let base: String = debian_name
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '@' | '.' | '_' | '+' | '-') {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let base = base.trim_matches('-').to_string();
    if base.ends_with(PACKAGE_SUFFIX) {
        base
    } else {
        format!("{base}{PACKAGE_SUFFIX}")
    }
}

pub struct PackageBuilder<'a> {
    deb: &'a DebPackage,
    options: BuildOptions,
}

impl<'a> PackageBuilder<'a> {
    pub fn new(deb: &'a DebPackage, options: BuildOptions) -> Self {
        Self { deb, options }
    }

    pub fn options(&self) -> &BuildOptions {
        &self.options
    }

    /// Melakukan konversi penuh tanpa pelaporan kemajuan.
    pub fn build(&self, deps: &[DepResolution]) -> Result<BuildResult> {
        self.build_with_progress(deps, &mut |_| {})
    }

    /// Melakukan konversi penuh, melaporkan kemajuan tiap tahap.
    pub fn build_with_progress(
        &self,
        deps: &[DepResolution],
        progress: &mut dyn FnMut(BuildStage),
    ) -> Result<BuildResult> {
        let control = &self.deb.control;
        let deb_name = control.package()?.to_string();
        let deb_version = control.version()?.to_string();
        let deb_arch = control.architecture()?.to_string();

        let pkgname = self
            .options
            .package_name
            .clone()
            .unwrap_or_else(|| arch_package_name(&deb_name));
        let version = convert_version(&deb_version);
        let arch = convert_arch(&deb_arch);

        fs::create_dir_all(&self.options.staging_dir)?;
        fs::create_dir_all(&self.options.output_dir)?;

        let mut notes = Vec::new();
        if arch != "x86_64" && arch != "any" {
            notes.push(format!(
                "arsitektur Debian `{deb_arch}` dipetakan ke `{arch}`; pastikan sesuai dengan sistem ini"
            ));
        }

        let mut extraction = self.extract_data(&mut notes, progress)?;
        if !extraction.forbidden.is_empty() {
            let list = extraction
                .forbidden
                .iter()
                .map(|p| format!("{} ({})", p.path, p.reason))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(Error::Build(format!(
                "paket menyentuh berkas sistem yang dilindungi: {list}"
            )));
        }

        let installed_size: u64 = extraction.entries.iter().map(|e| e.size).sum();
        let dependencies = dependency_names(deps, &pkgname, &deb_name);
        let script_report = scripts::analyze(&self.deb.maintainer_scripts());
        let install_script = if self.options.include_install_script {
            scripts::build_install_script(&script_report)
        } else {
            None
        };

        let pkginfo = self.render_pkginfo(&PkgInfoMeta {
            pkgname: &pkgname,
            deb_name: &deb_name,
            deb_version: &deb_version,
            version: &version,
            arch: &arch,
            dependencies: &dependencies,
            installed_size,
        });

        let staging = &self.options.staging_dir;
        self.write_metadata_file(staging, ".PKGINFO", pkginfo.as_bytes(), &mut extraction.entries)?;
        if let Some(script) = &install_script {
            self.write_metadata_file(staging, ".INSTALL", script.as_bytes(), &mut extraction.entries)?;
        }

        let mut mtree_buf = Vec::new();
        mtree::write_gzip(&extraction.entries, &mut mtree_buf)
            .map_err(|e| Error::Build(format!(".MTREE: {e}")))?;
        fs::write(staging.join(".MTREE"), &mtree_buf)?;

        let file_count = extraction
            .entries
            .iter()
            .filter(|e| e.kind == MtreeKind::File)
            .count();

        let file_name = format!("{pkgname}-{}-{arch}.pkg.tar.zst", version.full());
        let package_path = self.options.output_dir.join(&file_name);
        self.pack_staging(staging, &package_path, &extraction.entries, progress)?;

        Ok(BuildResult {
            package_path,
            pkgname,
            pkgver: version.pkgver,
            pkgrel: version.pkgrel,
            arch,
            installed_size,
            file_count,
            warnings: extraction.warnings,
            notes,
            install_script,
            dependencies,
        })
    }

    fn extract_data(
        &self,
        notes: &mut Vec<String>,
        progress: &mut dyn FnMut(BuildStage),
    ) -> Result<Extraction> {
        let staging = &self.options.staging_dir;
        let reader = self.deb.data_reader()?;
        let mut archive = tar::Archive::new(reader);

        let mut entries: Vec<MtreeEntry> = Vec::new();
        let mut warnings: Vec<SensitivePath> = Vec::new();
        let mut forbidden: Vec<SensitivePath> = Vec::new();
        let mut deferred_links: Vec<(String, String, u32, i64)> = Vec::new();
        let mut seen: BTreeMap<String, ()> = BTreeMap::new();

        for entry in archive.entries()? {
            let mut entry = entry?;
            let header = entry.header();
            let entry_type = header.entry_type();
            let raw_path = entry.path()?.to_string_lossy().to_string();
            let path = normalize::normalize_path(&raw_path);
            if path == "/" || path.is_empty() {
                continue;
            }
            let rel = path.trim_start_matches('/').to_string();
            let mode = header.mode().unwrap_or(0o644);
            let mtime = header.mtime().unwrap_or(0) as i64;

            if let Some(hit) = security::check_path(&path) {
                match hit.level {
                    security::Sensitivity::Forbidden => {
                        forbidden.push(hit);
                        continue;
                    }
                    security::Sensitivity::Warning => warnings.push(hit),
                }
            }

            if entries.len().is_multiple_of(32) {
                progress(BuildStage::Extracting {
                    files: entries.len(),
                });
            }

            if seen.insert(rel.clone(), ()).is_some() {
                // Direktori duplikat normal terjadi saat multiarch digabung
                // (mis. /usr/lib/x86_64-linux-gnu dan /usr/lib).
                if !entry_type.is_dir() {
                    notes.push(format!(
                        "dua berkas menuju path yang sama setelah normalisasi: {path}"
                    ));
                }
                continue;
            }

            if entry_type.is_dir() {
                ensure_dir(staging, &rel)?;
                entries.push(MtreeEntry {
                    path: rel,
                    kind: MtreeKind::Dir,
                    mode: 0o755,
                    size: 0,
                    time: mtime,
                    md5: None,
                    sha256: None,
                    link: None,
                });
                continue;
            }

            if entry_type.is_symlink() {
                let target = entry
                    .link_name()?
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                let normalized = normalize::normalize_link_target(&target);
                let dest = staging.join(&rel);
                ensure_parent(staging, &rel)?;
                if dest.symlink_metadata().is_ok() {
                    fs::remove_file(&dest)?;
                }
                std::os::unix::fs::symlink(&normalized, &dest)?;
                entries.push(MtreeEntry {
                    path: rel,
                    kind: MtreeKind::Link,
                    mode: 0o777,
                    size: 0,
                    time: mtime,
                    md5: None,
                    sha256: None,
                    link: Some(normalized),
                });
                continue;
            }

            if entry_type.is_hard_link() {
                let target = entry
                    .link_name()?
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                let target_rel = normalize::normalize_path(&target)
                    .trim_start_matches('/')
                    .to_string();
                deferred_links.push((rel, target_rel, mode, mtime));
                continue;
            }

            if !entry_type.is_file() {
                // FIFO, socket, device: tidak boleh masuk paket.
                notes.push(format!("entri bertipe khusus dilewati: {path}"));
                continue;
            }

            ensure_parent(staging, &rel)?;
            let dest = staging.join(&rel);
            if dest.exists() {
                return Err(Error::Build(format!(
                    "konflik berkas: {path} sudah ada di paket (kemungkinan duplikat setelah normalisasi)"
                )));
            }
            let (size, md5_hex, sha256_hex) = write_file(&mut entry, &dest, mode)?;
            entries.push(MtreeEntry {
                path: rel,
                kind: MtreeKind::File,
                mode: mode & 0o7777,
                size,
                time: mtime,
                md5: Some(md5_hex),
                sha256: Some(sha256_hex),
                link: None,
            });
        }

        // Hardlink: target harus sudah diekstrak.
        for (rel, target_rel, mode, mtime) in deferred_links {
            let target = staging.join(&target_rel);
            let dest = staging.join(&rel);
            ensure_parent(staging, &rel)?;
            let meta = target.symlink_metadata().map_err(|_| {
                Error::Build(format!(
                    "hardlink {} menunjuk target yang tidak ada: {}",
                    rel, target_rel
                ))
            })?;
            if meta.is_dir() {
                return Err(Error::Build(format!(
                    "hardlink {} menunjuk direktori: {}",
                    rel, target_rel
                )));
            }
            fs::hard_link(&target, &dest).or_else(|_| fs::copy(&target, &dest).map(|_| ()))?;
            let data = fs::read(&dest)?;
            entries.push(MtreeEntry {
                path: rel,
                kind: MtreeKind::File,
                mode: mode & 0o7777,
                size: data.len() as u64,
                time: mtime,
                md5: Some(format!("{:x}", Md5::digest(&data))),
                sha256: Some(format!("{:x}", Sha256::digest(&data))),
                link: None,
            });
        }

        Ok(Extraction {
            entries,
            warnings,
            forbidden,
        })
    }

    fn write_metadata_file(
        &self,
        staging: &Path,
        name: &str,
        content: &[u8],
        entries: &mut Vec<MtreeEntry>,
    ) -> Result<()> {
        let path = staging.join(name);
        fs::write(&path, content)?;
        let mode = 0o644;
        fs::set_permissions(&path, fs::Permissions::from_mode(mode))?;
        entries.push(MtreeEntry {
            path: name.to_string(),
            kind: MtreeKind::File,
            mode,
            size: content.len() as u64,
            time: now_secs(),
            md5: Some(format!("{:x}", Md5::digest(content))),
            sha256: Some(format!("{:x}", Sha256::digest(content))),
            link: None,
        });
        Ok(())
    }

    fn render_pkginfo(&self, meta: &PkgInfoMeta<'_>) -> String {
        let PkgInfoMeta {
            pkgname,
            deb_name,
            deb_version,
            version,
            arch,
            dependencies,
            installed_size,
        } = *meta;
        let control: &Control = &self.deb.control;
        let mut out = String::new();
        out.push_str("# Dibuat oleh DebInstaller\n");
        out.push_str(&format!("pkgname = {pkgname}\n"));
        out.push_str(&format!("pkgbase = {pkgname}\n"));
        out.push_str(&format!("pkgver = {}\n", version.full()));
        out.push_str(&format!("pkgdesc = {}\n", single_line(control.synopsis())));
        out.push_str(&format!(
            "url = {}\n",
            control.get("homepage").unwrap_or("").trim()
        ));
        out.push_str(&format!("builddate = {}\n", now_secs()));
        out.push_str(&format!("packager = {}\n", single_line(&self.options.packager)));
        out.push_str(&format!("size = {installed_size}\n"));
        out.push_str(&format!("arch = {arch}\n"));
        out.push_str("license = custom\n");
        for dep in dependencies {
            out.push_str(&format!("depend = {dep}\n"));
        }
        out.push_str(&format!("xdata = debian-package={deb_name}\n"));
        out.push_str(&format!("xdata = debian-version={deb_version}\n"));
        out.push_str(&format!("xdata = debian-sha256={}\n", self.deb.sha256));
        out.push_str(&format!("xdata = debian-arch={}\n", control.architecture().unwrap_or("")));
        out
    }

    fn pack_staging(
        &self,
        staging: &Path,
        out_path: &Path,
        entries: &[MtreeEntry],
        progress: &mut dyn FnMut(BuildStage),
    ) -> Result<()> {
        let mut paths = Vec::new();
        collect_paths(staging, staging, &mut paths)?;
        paths.sort();

        // Mtime dan mode harus sama dengan yang tercatat di `.MTREE`, kalau
        // tidak `pacman -Qkk` akan melaporkan berkas berubah.
        let metadata: BTreeMap<&str, &MtreeEntry> =
            entries.iter().map(|e| (e.path.as_str(), e)).collect();

        // Perkiraan ukuran tar: isi berkas + header 512 byte per entri.
        let payload: u64 = entries
            .iter()
            .filter(|e| e.kind == MtreeKind::File)
            .map(|e| e.size)
            .sum();
        let total = payload + (paths.len() as u64 + 1) * 512;

        let file = fs::File::create(out_path)?;
        let mut encoder = zstd::stream::write::Encoder::new(file, self.options.compression_level)
            .map_err(|e| Error::Build(format!("zstd: {e}")))?;

        let workers = match self.options.compression_threads {
            0 => std::thread::available_parallelism()
                .map(|n| n.get().min(8) as u32)
                .unwrap_or(1),
            n => n,
        };
        if workers > 1 {
            encoder
                .multithread(workers)
                .map_err(|e| Error::Build(format!("zstd multithread: {e}")))?;
        }

        let counter = ProgressWriter::new(encoder, total, progress);
        let mut builder = tar::Builder::new(counter);
        builder.mode(tar::HeaderMode::Deterministic);

        for rel in &paths {
            let abs = staging.join(rel);
            let meta = abs.symlink_metadata()?;
            let recorded = metadata.get(rel.as_str());
            let mtime = recorded.map(|e| e.time).unwrap_or_else(now_secs);
            let mode = recorded
                .map(|e| e.mode)
                .unwrap_or_else(|| meta.permissions().mode() & 0o7777);

            let mut header = tar::Header::new_gnu();
            header.set_uid(0);
            header.set_gid(0);
            header.set_username("root").ok();
            header.set_groupname("root").ok();
            header.set_mtime(mtime.max(0) as u64);
            header.set_mode(mode);

            if meta.is_symlink() {
                let target = fs::read_link(&abs)?;
                header.set_entry_type(tar::EntryType::Symlink);
                header.set_size(0);
                header.set_link_name(&target)?;
                header.set_cksum();
                builder.append_data(&mut header, rel, std::io::empty())?;
            } else if meta.is_dir() {
                header.set_entry_type(tar::EntryType::Directory);
                header.set_size(0);
                header.set_cksum();
                // Direktori disimpan dengan akhiran `/`, seperti paket
                // buatan makepkg; pacman memakai akhiran ini untuk
                // mengenali direktori di daftar berkas.
                let dir_name = format!("{rel}/");
                builder.append_data(&mut header, dir_name, std::io::empty())?;
            } else {
                header.set_entry_type(tar::EntryType::Regular);
                header.set_size(meta.len());
                header.set_cksum();
                let mut f = fs::File::open(&abs)?;
                builder.append_data(&mut header, rel, &mut f)?;
            }
        }

        let counter = builder
            .into_inner()
            .map_err(|e| Error::Build(format!("tar: {e}")))?;
        counter
            .into_inner()
            .finish()
            .map_err(|e| Error::Build(format!("zstd: {e}")))?;
        Ok(())
    }
}

/// Writer yang menghitung byte tar (sebelum dikompresi) untuk pelaporan
/// kemajuan, lalu meneruskannya ke encoder zstd.
struct ProgressWriter<'a, W: Write> {
    inner: W,
    written: u64,
    total: u64,
    next_report: u64,
    progress: &'a mut dyn FnMut(BuildStage),
}

impl<'a, W: Write> ProgressWriter<'a, W> {
    /// Ambang pelaporan: 8 MiB atau 1/100 total, mana yang lebih besar.
    fn new(inner: W, total: u64, progress: &'a mut dyn FnMut(BuildStage)) -> Self {
        let step = (total / 100).max(8 * 1024 * 1024);
        Self {
            inner,
            written: 0,
            total,
            next_report: step,
            progress,
        }
    }

    fn into_inner(self) -> W {
        self.inner
    }
}

impl<W: Write> Write for ProgressWriter<'_, W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.written += n as u64;
        if self.written >= self.next_report {
            (self.progress)(BuildStage::Packing {
                written: self.written.min(self.total),
                total: self.total,
            });
            self.next_report = self.written + (self.total / 100).max(8 * 1024 * 1024);
        }
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// Data yang dibutuhkan untuk menulis `.PKGINFO`.
struct PkgInfoMeta<'a> {
    pkgname: &'a str,
    deb_name: &'a str,
    deb_version: &'a str,
    version: &'a ArchVersion,
    arch: &'a str,
    dependencies: &'a [String],
    installed_size: u64,
}

struct Extraction {
    entries: Vec<MtreeEntry>,
    warnings: Vec<SensitivePath>,
    forbidden: Vec<SensitivePath>,
}

/// Nama dependensi yang akan ditulis ke `.PKGINFO`, tanpa paket itu sendiri.
fn dependency_names(deps: &[DepResolution], pkgname: &str, deb_name: &str) -> Vec<String> {
    let mut out: Vec<String> = deps
        .iter()
        .filter(|d| matches!(d.status, DepStatus::Installed | DepStatus::Available))
        .filter_map(|d| d.arch_name.clone())
        .filter(|name| name != pkgname && name != deb_name)
        .collect();
    out.sort();
    out.dedup();
    out
}

fn single_line(input: &str) -> String {
    input.replace('\n', " ").trim().to_string()
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Membuat direktori untuk `rel`, menolak bila ada komponen berupa symlink
/// (mencegah penulisan keluar dari staging).
fn ensure_parent(staging: &Path, rel: &str) -> Result<()> {
    match rel.rsplit_once('/') {
        Some((parent, _)) => ensure_dir(staging, parent),
        None => Ok(()),
    }
}

fn ensure_dir(staging: &Path, rel: &str) -> Result<()> {
    let mut current = staging.to_path_buf();
    for part in rel.split('/').filter(|p| !p.is_empty() && *p != ".") {
        current.push(part);
        match current.symlink_metadata() {
            Ok(meta) if meta.is_dir() => {}
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(Error::UnsafePath(format!(
                    "{} adalah symlink; paket tidak boleh menulis melewatinya",
                    current.display()
                )))
            }
            Ok(_) => {
                return Err(Error::Build(format!(
                    "{} sudah ada sebagai berkas, bukan direktori",
                    current.display()
                )))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&current)?;
            }
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

fn write_file(reader: &mut impl Read, dest: &Path, mode: u32) -> Result<(u64, String, String)> {
    let mut out = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dest)?;
    let mut md5 = Md5::new();
    let mut sha = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    let mut size = 0u64;
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        md5.update(&buf[..n]);
        sha.update(&buf[..n]);
        out.write_all(&buf[..n])?;
        size += n as u64;
    }
    out.flush()?;
    drop(out);
    fs::set_permissions(dest, fs::Permissions::from_mode(mode & 0o7777))?;
    Ok((size, format!("{:x}", md5.finalize()), format!("{:x}", sha.finalize())))
}

fn collect_paths(root: &Path, current: &Path, out: &mut Vec<String>) -> Result<()> {
    let mut entries: Vec<_> = fs::read_dir(current)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        let rel = path
            .strip_prefix(root)
            .map_err(|_| Error::Build("path staging tidak valid".into()))?
            .to_string_lossy()
            .to_string();
        let meta = path.symlink_metadata()?;
        if meta.is_dir() {
            out.push(rel.clone());
            collect_paths(root, &path, out)?;
        } else {
            out.push(rel);
        }
    }
    Ok(())
}

/// Ringkasan hasil analisis script untuk ditampilkan di UI.
pub fn script_summary(report: &ScriptReport) -> String {
    if report.findings.is_empty() {
        return "Tidak ada perintah bermasalah pada script maintainer.".to_string();
    }
    format!(
        "{} temuan, risiko tertinggi: {}",
        report.findings.len(),
        report.max_risk().label()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_debian_versions() {
        let v = convert_version("1.90.0-1");
        assert_eq!(v.pkgver, "1.90.0");
        assert_eq!(v.pkgrel, "1");
        assert_eq!(v.full(), "1.90.0-1");

        let v = convert_version("2:3.4.5~beta1-0ubuntu2");
        assert_eq!(v.epoch, Some(2));
        assert_eq!(v.pkgver, "3.4.5~beta1");
        assert_eq!(v.pkgrel, "0.2");
        assert_eq!(v.full(), "2:3.4.5~beta1-0.2");

        let v = convert_version("1.0");
        assert_eq!(v.pkgrel, "1");
        assert_eq!(v.epoch, None);

        let v = convert_version("1.0-1:weird");
        assert!(v.epoch.is_none());
    }

    #[test]
    fn sanitizes_versions_for_pacman() {
        let v = convert_version("1.0 beta+git-1_2");
        assert!(!v.pkgver.contains(' '));
        assert_eq!(v.pkgrel, "1.2");
        let v = convert_version("1.0-");
        assert_eq!(v.pkgrel, "1");
    }

    #[test]
    fn converts_architectures() {
        assert_eq!(convert_arch("amd64"), "x86_64");
        assert_eq!(convert_arch("arm64"), "aarch64");
        assert_eq!(convert_arch("all"), "any");
        assert_eq!(convert_arch("i386"), "i686");
        assert_eq!(convert_arch("riscv64"), "riscv64");
    }

    #[test]
    fn names_packages_with_suffix() {
        assert_eq!(arch_package_name("code"), "code-deb");
        assert_eq!(arch_package_name("Code-OSS"), "code-oss-deb");
        assert_eq!(arch_package_name("already-deb"), "already-deb");
        assert_eq!(arch_package_name("weird name!"), "weird-name-deb");
    }

    #[test]
    fn dependency_names_filters_unresolved() {
        let mk = |name: &str, arch: Option<&str>, status: DepStatus| DepResolution {
            debian: name.into(),
            constraint: None,
            alternatives: vec![name.into()],
            arch_name: arch.map(str::to_string),
            status,
            confidence: crate::deps::Confidence::High,
            source: crate::deps::ResolutionSource::Mapping,
            note: None,
        };
        let deps = vec![
            mk("a", Some("glibc"), DepStatus::Available),
            mk("b", Some("gtk3"), DepStatus::Installed),
            mk("c", Some("aur-thing"), DepStatus::AurOnly),
            mk("d", None, DepStatus::Missing),
            mk("e", Some("glibc"), DepStatus::Installed),
        ];
        assert_eq!(
            dependency_names(&deps, "app-deb", "app"),
            vec!["glibc", "gtk3"]
        );
        // Paket tidak boleh bergantung pada dirinya sendiri.
        let self_dep = vec![mk("a", Some("app-deb"), DepStatus::Available)];
        assert!(dependency_names(&self_dep, "app-deb", "app").is_empty());
    }

    #[test]
    fn rejects_metadata_path_escape() {
        let dir = tempfile::tempdir().unwrap();
        let staging = dir.path().join("stage");
        fs::create_dir_all(&staging).unwrap();
        fs::create_dir_all(staging.join("usr")).unwrap();
        // Simulasi: paket punya symlink usr/bin → /etc, lalu menulis lewat sana.
        std::os::unix::fs::symlink("/etc", staging.join("usr/bin")).unwrap();
        let err = ensure_dir(&staging, "usr/bin/evil").unwrap_err();
        assert!(matches!(err, Error::UnsafePath(_)));
    }
}
