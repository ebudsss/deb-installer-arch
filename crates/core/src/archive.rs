//! Dukungan arsip tar sebagai fitur tambahan: `.tar.gz`, `.tgz`, `.tar.xz`,
//! `.tar.zst`.
//!
//! Arsip tidak punya metadata seperti `.deb`, jadi DebInstaller menebak nama
//! dan versi dari nama berkas, lalu memilih salah satu dari dua tata letak:
//!
//! * **Sistem** — arsip sudah berisi `usr/`, `opt/`, `etc/`, atau `var/` di
//!   akar; isinya dipakai apa adanya setelah normalisasi path.
//! * **Portable** — arsip berisi satu direktori akar atau berkas lepas; isinya
//!   dipasang ke `/opt/<paket>/`, binari di `bin/` (dan di akar) ditautkan ke
//!   `/usr/bin`, dan berkas `.desktop` dipindahkan ke
//!   `/usr/share/applications`.
//!
//! Kedua jalur berakhir sebagai paket pacman, sehingga instalasi, pelacakan,
//! dan penghapusan memakai mekanisme yang sama dengan konversi `.deb`.

use crate::deb::{self, Compression};
use crate::error::{Error, Result};
use crate::mtree::{self, MtreeEntry, MtreeKind};
use crate::pkgbuild::{
    self, arch_package_name, extract_tar, pack_staging, render_pkginfo, BuildOptions, BuildResult,
    BuildStage, PathPlan, PkgInfoMeta,
};
use crate::security::SensitivePath;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Tata letak yang terdeteksi di dalam arsip.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ArchiveLayout {
    /// Arsip sudah memakai tata letak sistem.
    System,
    /// Arsip portable; `root` adalah direktori akar yang dibuang.
    Portable { root: Option<String> },
}

impl ArchiveLayout {
    pub fn label(&self) -> &'static str {
        match self {
            ArchiveLayout::System => "sistem",
            ArchiveLayout::Portable { .. } => "portable",
        }
    }
}

/// Satu entri arsip, untuk ditampilkan di UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveEntry {
    pub path: String,
    pub kind: &'static str,
    pub size: u64,
    pub executable: bool,
}

/// Arsip tar yang sudah dianalisis.
#[derive(Debug, Clone)]
pub struct ArchivePackage {
    pub path: PathBuf,
    pub file_size: u64,
    pub sha256: String,
    /// Nama paket yang ditebak dari nama berkas (tanpa akhiran `-deb`).
    pub name: String,
    /// Versi yang ditebak; `1.0.0` bila tidak ada pola versi.
    pub version: String,
    pub layout: ArchiveLayout,
    pub entries: Vec<ArchiveEntry>,
    /// Deskripsi dari berkas `.desktop`, bila ada.
    pub description: Option<String>,
    /// Nama binari yang akan ditautkan ke `/usr/bin` (hanya portable).
    pub executables: Vec<String>,
    compression: Compression,
}

impl ArchivePackage {
    /// Membuka arsip dan membaca seluruh daftar isinya. Arsip tar harus
    /// dibaca dua kali (deteksi lalu ekstraksi), jadi pembacaan di sini hanya
    /// untuk metadata dan validasi.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let meta = fs::metadata(&path)?;
        if !meta.is_file() {
            return Err(Error::Other(format!("{} bukan file biasa", path.display())));
        }
        let compression = compression_for(&path)?;
        let sha256 = deb::hash_file(&path)?;

        let mut reader = decoder(&path, compression)?;
        let mut archive = tar::Archive::new(&mut reader);
        let mut paths: Vec<(String, &'static str, u64, bool)> = Vec::new();
        let mut desktop_candidates: Vec<(String, Vec<u8>)> = Vec::new();

        for entry in archive.entries()? {
            let mut entry = entry?;
            let header = entry.header();
            let kind = header.entry_type();
            let raw = entry.path()?.to_string_lossy().to_string();
            let trimmed = raw
                .trim_start_matches("./")
                .trim_start_matches('/')
                .to_string();
            if trimmed.is_empty() {
                continue;
            }
            let size = header.size().unwrap_or(0);
            let mode = header.mode().unwrap_or(0o644);
            let label = if kind.is_dir() {
                "dir"
            } else if kind.is_symlink() {
                "symlink"
            } else if kind.is_hard_link() {
                "hardlink"
            } else {
                "file"
            };
            let executable = kind.is_file() && mode & 0o111 != 0;
            if label == "file" && trimmed.ends_with(".desktop") && size < 64 * 1024 {
                let mut buf = Vec::new();
                let _ = entry.read_to_end(&mut buf);
                desktop_candidates.push((trimmed.clone(), buf));
            }
            paths.push((trimmed, label, size, executable));
        }

        if paths.is_empty() {
            return Err(Error::UnsupportedArchive(
                "arsip tidak berisi berkas".into(),
            ));
        }

        let layout = detect_layout(&paths);
        let executables = match &layout {
            ArchiveLayout::System => Vec::new(),
            ArchiveLayout::Portable { root } => detect_executables(&paths, root.as_deref()),
        };
        let description = desktop_candidates
            .iter()
            .find_map(|(_, bytes)| desktop_field(bytes, "Comment"));

        let name = package_name_from_path(&path);
        let version = version_from_path(&path).unwrap_or_else(|| "1.0.0".to_string());

        Ok(Self {
            path,
            file_size: meta.len(),
            sha256,
            name,
            version,
            layout,
            entries: paths
                .into_iter()
                .map(|(path, kind, size, executable)| ArchiveEntry {
                    path,
                    kind,
                    size,
                    executable,
                })
                .collect(),
            description,
            executables,
            compression,
        })
    }

    /// Nama paket pacman yang disarankan.
    pub fn suggested_pkgname(&self) -> String {
        arch_package_name(&self.name)
    }

    /// Membangun paket pacman dari arsip.
    pub fn build(&self, options: &BuildOptions) -> Result<BuildResult> {
        self.build_with_progress(options, &mut |_| {})
    }

    pub fn build_with_progress(
        &self,
        options: &BuildOptions,
        progress: &mut dyn FnMut(BuildStage),
    ) -> Result<BuildResult> {
        let pkgname = options
            .package_name
            .clone()
            .unwrap_or_else(|| self.suggested_pkgname());
        let version = pkgbuild::convert_version(&self.version);
        let arch = pkgbuild::convert_arch("amd64");

        fs::create_dir_all(&options.staging_dir)?;
        fs::create_dir_all(&options.output_dir)?;

        let mut notes = Vec::new();
        let plan = match &self.layout {
            ArchiveLayout::System => {
                notes.push(
                    "arsip sudah memakai tata letak sistem; isi dipakai apa adanya".to_string(),
                );
                PathPlan::System
            }
            ArchiveLayout::Portable { root } => {
                notes.push(format!(
                    "arsip portable dipasang ke /opt/{pkgname} dengan tautan binari di /usr/bin"
                ));
                PathPlan::Portable {
                    root: root.clone(),
                    prefix: format!("opt/{pkgname}"),
                }
            }
        };

        let reader = decoder(&self.path, self.compression)?;
        let mut extraction = extract_tar(reader, &options.staging_dir, plan, &mut notes, progress)?;

        if !extraction.forbidden.is_empty() {
            let list = extraction
                .forbidden
                .iter()
                .map(|p| format!("{} ({})", p.path, p.reason))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(Error::Build(format!(
                "arsip menyentuh berkas sistem yang dilindungi: {list}"
            )));
        }

        if self.executables.is_empty() && matches!(self.layout, ArchiveLayout::Portable { .. }) {
            notes.push(
                "tidak ada binari yang bisa ditautkan ke /usr/bin; jalankan dari /opt secara manual"
                    .to_string(),
            );
        }

        let installed_size: u64 = extraction.entries.iter().map(|e| e.size).sum();
        let full_version = version.full();
        let archive_name = self
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let xdata = vec![
            ("archive-name".to_string(), archive_name),
            ("archive-sha256".to_string(), self.sha256.clone()),
            (
                "archive-layout".to_string(),
                self.layout.label().to_string(),
            ),
        ];
        let dependencies: Vec<String> = Vec::new();
        let pkginfo = render_pkginfo(&PkgInfoMeta {
            pkgname: &pkgname,
            pkgver: &full_version,
            desc: self.description.as_deref().unwrap_or(""),
            url: "",
            arch: &arch,
            packager: &options.packager,
            dependencies: &dependencies,
            installed_size,
            xdata: &xdata,
        });

        let staging = &options.staging_dir;
        write_metadata(
            staging,
            ".PKGINFO",
            pkginfo.as_bytes(),
            &mut extraction.entries,
        )?;

        let mut mtree_buf = Vec::new();
        mtree::write_gzip(&extraction.entries, &mut mtree_buf)
            .map_err(|e| Error::Build(format!(".MTREE: {e}")))?;
        fs::write(staging.join(".MTREE"), &mtree_buf)?;

        // Berkas metadata pacman (`.PKGINFO`, `.INSTALL`) tidak dihitung
        // sebagai berkas paket agar angka sama dengan daftar isi.
        let file_count = extraction
            .entries
            .iter()
            .filter(|e| e.kind == MtreeKind::File && !e.path.starts_with('.'))
            .count();

        let file_name = format!("{pkgname}-{}-{arch}.pkg.tar.zst", version.full());
        let package_path = options.output_dir.join(&file_name);
        pack_staging(
            staging,
            &package_path,
            &extraction.entries,
            options.compression_level,
            options.compression_threads,
            progress,
        )?;

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
            install_script: None,
            dependencies,
        })
    }
}

fn write_metadata(
    staging: &Path,
    name: &str,
    content: &[u8],
    entries: &mut Vec<MtreeEntry>,
) -> Result<()> {
    use md5::Md5;
    use sha2::{Digest, Sha256};
    use std::os::unix::fs::PermissionsExt;

    let path = staging.join(name);
    fs::write(&path, content)?;
    let mode = 0o644;
    fs::set_permissions(&path, fs::Permissions::from_mode(mode))?;
    entries.push(MtreeEntry {
        path: name.to_string(),
        kind: MtreeKind::File,
        mode,
        size: content.len() as u64,
        time: crate::state::now_secs(),
        md5: Some(format!("{:x}", Md5::digest(content))),
        sha256: Some(format!("{:x}", Sha256::digest(content))),
        link: None,
    });
    Ok(())
}

/// Kompresi berdasarkan nama berkas arsip.
pub fn compression_for(path: &Path) -> Result<Compression> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let compression = if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        Compression::Gzip
    } else if name.ends_with(".tar.xz") || name.ends_with(".txz") {
        Compression::Xz
    } else if name.ends_with(".tar.zst") || name.ends_with(".tzst") {
        Compression::Zstd
    } else if name.ends_with(".tar") {
        Compression::None
    } else {
        return Err(Error::UnsupportedArchive(format!(
            "`{}` bukan arsip tar yang didukung (gunakan .tar.gz, .tgz, .tar.xz, atau .tar.zst)",
            path.display()
        )));
    };
    Ok(compression)
}

fn decoder(path: &Path, compression: Compression) -> Result<Box<dyn Read>> {
    let file = fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);
    Ok(match compression {
        Compression::None => Box::new(reader),
        Compression::Gzip => Box::new(flate2::read::GzDecoder::new(reader)),
        Compression::Xz => Box::new(xz2::read::XzDecoder::new(reader)),
        Compression::Zstd => Box::new(
            zstd::stream::read::Decoder::new(reader)
                .map_err(|e| Error::UnsupportedArchive(format!("zstd: {e}")))?,
        ),
    })
}

/// Apakah path terlihat seperti bagian dari tata letak sistem.
fn is_system_root(component: &str) -> bool {
    matches!(
        component,
        "usr" | "opt" | "etc" | "var" | "lib" | "bin" | "sbin"
    )
}

fn detect_layout(paths: &[(String, &'static str, u64, bool)]) -> ArchiveLayout {
    let roots: BTreeSet<&str> = paths
        .iter()
        .filter_map(|(path, ..)| path.split('/').next())
        .collect();

    if roots.iter().any(|root| is_system_root(root)) {
        return ArchiveLayout::System;
    }

    // Satu direktori akar membungkus seluruh arsip → portable dengan root.
    if roots.len() == 1 {
        let root = *roots.iter().next().expect("satu elemen");
        let all_inside = paths
            .iter()
            .all(|(path, ..)| path == root || path.starts_with(&format!("{root}/")));
        let root_is_dir = paths
            .iter()
            .any(|(path, kind, ..)| path == root && *kind == "dir");
        if all_inside && (root_is_dir || paths.iter().all(|(p, ..)| p.contains('/'))) {
            return ArchiveLayout::Portable {
                root: Some(root.to_string()),
            };
        }
    }

    ArchiveLayout::Portable { root: None }
}

/// Binari yang layak ditautkan ke `/usr/bin`: berkas executable di `bin/`
/// atau di akar arsip (setelah root dibuang).
fn detect_executables(
    paths: &[(String, &'static str, u64, bool)],
    root: Option<&str>,
) -> Vec<String> {
    let mut names: Vec<String> = paths
        .iter()
        .filter(|(_, kind, _, executable)| *kind == "file" && *executable)
        .filter_map(|(path, ..)| {
            let stripped = match root {
                Some(root) => path.strip_prefix(&format!("{root}/"))?,
                None => path.as_str(),
            };
            let in_bin = stripped.starts_with("bin/");
            let at_root = !stripped.contains('/');
            if !(in_bin || at_root) {
                return None;
            }
            let name = stripped.rsplit('/').next()?.to_string();
            // Berkas seperti `install.sh` tetap ditautkan; pengguna melihat
            // daftarnya sebelum memasang.
            Some(name)
        })
        .collect();
    names.sort();
    names.dedup();
    names
}

/// Nama paket dari nama berkas: `myapp-1.2.3-linux-x64.tar.gz` → `myapp`.
pub fn package_name_from_path(path: &Path) -> String {
    let stem = strip_archive_suffix(
        &path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
    );
    let mut parts = stem.split(['-', '_']);
    let head = parts.next().unwrap_or(&stem).to_string();
    // Buang bagian setelah segmen pertama yang mengandung angka, karena
    // biasanya itu versi atau arsitektur.
    let mut name = head;
    for part in parts {
        if part.chars().any(|c| c.is_ascii_digit()) {
            break;
        }
        name.push('-');
        name.push_str(part);
    }
    let sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-') {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let sanitized = sanitized.trim_matches('-').to_string();
    if sanitized.is_empty() {
        "archive".to_string()
    } else {
        sanitized
    }
}

/// Versi dari nama berkas: `myapp-1.2.3-linux-x64.tar.gz` → `1.2.3`.
pub fn version_from_path(path: &Path) -> Option<String> {
    let stem = strip_archive_suffix(
        &path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
    );
    let candidate = stem.split(['-', '_']).find(|part| {
        let mut chars = part.chars();
        matches!(chars.next(), Some(c) if c.is_ascii_digit()) && part.chars().any(|c| c == '.')
    })?;
    let cleaned: String = candidate
        .chars()
        .take_while(|c| c.is_ascii_digit() || matches!(c, '.' | '_' | '+'))
        .collect();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

fn strip_archive_suffix(name: &str) -> String {
    let lower = name.to_lowercase();
    for suffix in [
        ".tar.gz", ".tar.xz", ".tar.zst", ".tgz", ".txz", ".tzst", ".tar",
    ] {
        if lower.ends_with(suffix) {
            return name[..name.len() - suffix.len()].to_string();
        }
    }
    name.to_string()
}

/// Membaca satu field dari berkas `.desktop`.
fn desktop_field(content: &[u8], key: &str) -> Option<String> {
    let text = String::from_utf8_lossy(content);
    let mut in_entry = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        if let Some((name, value)) = line.split_once('=') {
            if name.trim() == key {
                let value = value.trim();
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}

/// Ringkasan peringatan untuk UI.
pub fn warning_count(warnings: &[SensitivePath]) -> usize {
    warnings.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_name_and_version_from_filename() {
        let p = Path::new("omniget-1.0.1-linux-x64.tar.gz");
        assert_eq!(package_name_from_path(p), "omniget");
        assert_eq!(version_from_path(p).as_deref(), Some("1.0.1"));

        let p = Path::new("MyApp_2.3.0_amd64.tar.xz");
        assert_eq!(package_name_from_path(p), "myapp");
        assert_eq!(version_from_path(p).as_deref(), Some("2.3.0"));
    }

    #[test]
    fn handles_archive_without_version() {
        let p = Path::new("tool.tar.zst");
        assert_eq!(package_name_from_path(p), "tool");
        assert_eq!(version_from_path(p), None);
    }

    #[test]
    fn detects_system_layout() {
        let entries = vec![
            ("usr/bin/app".to_string(), "file", 10, true),
            ("usr/share/doc/app/README".to_string(), "file", 10, false),
        ];
        assert_eq!(detect_layout(&entries), ArchiveLayout::System);
    }

    #[test]
    fn detects_portable_layout_with_root() {
        let entries = vec![
            ("myapp".to_string(), "dir", 0, false),
            ("myapp/bin/myapp".to_string(), "file", 10, true),
            ("myapp/README.md".to_string(), "file", 10, false),
        ];
        assert_eq!(
            detect_layout(&entries),
            ArchiveLayout::Portable {
                root: Some("myapp".to_string())
            }
        );
    }

    #[test]
    fn detects_flat_portable_layout() {
        let entries = vec![
            ("tool".to_string(), "file", 10, true),
            ("LICENSE".to_string(), "file", 10, false),
        ];
        assert_eq!(
            detect_layout(&entries),
            ArchiveLayout::Portable { root: None }
        );
    }

    #[test]
    fn finds_executables_inside_root() {
        let entries = vec![
            ("app".to_string(), "dir", 0, false),
            ("app/bin/run".to_string(), "file", 10, true),
            ("app/bin/helper".to_string(), "file", 10, true),
            ("app/lib/libx.so".to_string(), "file", 10, true),
            ("app/README.md".to_string(), "file", 10, false),
        ];
        assert_eq!(
            detect_executables(&entries, Some("app")),
            vec!["helper".to_string(), "run".to_string()]
        );
    }

    #[test]
    fn reads_desktop_comment() {
        let content = b"[Desktop Entry]\nType=Application\nName=App\nComment=Does a thing\n";
        assert_eq!(
            desktop_field(content, "Comment").as_deref(),
            Some("Does a thing")
        );
        assert_eq!(desktop_field(content, "Name").as_deref(), Some("App"));
        assert_eq!(desktop_field(content, "Icon"), None);
    }

    #[test]
    fn rejects_unsupported_extension() {
        assert!(compression_for(Path::new("x.zip")).is_err());
        assert_eq!(
            compression_for(Path::new("x.tgz")).unwrap(),
            Compression::Gzip
        );
    }
}
