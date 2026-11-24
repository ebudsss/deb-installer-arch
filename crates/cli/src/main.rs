//! CLI `deb-installer`: alat uji dan otomasi untuk konversi `.deb` → pacman.

use clap::{Parser, Subcommand};
use deb_installer_core::archive::{self, ArchiveLayout, ArchivePackage};
use deb_installer_core::control::VersionOp;
use deb_installer_core::deb::{DebPackage, FileKind};
use deb_installer_core::deps::{DepMapper, MappingTable, SystemPacman};
use deb_installer_core::install::PacmanInstaller;
use deb_installer_core::pkgbuild::{BuildOptions, PackageBuilder};
use deb_installer_core::scripts;
use deb_installer_core::security;
use deb_installer_core::state::{self, History, InstallRecord};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "deb-installer",
    version,
    about = "Konversi paket .deb menjadi paket pacman native",
    long_about = "Membaca .deb, memetakan dependensi Debian ke Arch, menganalisis script \
maintainer, lalu membangun .pkg.tar.zst yang bisa dipasang dengan pacman -U."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Tampilkan metadata paket
    Info {
        file: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Daftar berkas yang akan terpasang (setelah normalisasi path)
    Files { file: PathBuf },
    /// Petakan dependensi Debian ke paket Arch
    Deps {
        file: PathBuf,
        /// Jangan tanya AUR (tanpa jaringan)
        #[arg(long)]
        no_network: bool,
        /// Berkas mapping tambahan (TOML)
        #[arg(long)]
        mapping: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Analisis script maintainer
    Scripts { file: PathBuf },
    /// Konversi .deb menjadi .pkg.tar.zst
    Build {
        file: PathBuf,
        /// Direktori keluaran paket
        #[arg(short, long, default_value = ".")]
        output: PathBuf,
        /// Direktori staging (default: direktori sementara)
        #[arg(long)]
        staging: Option<PathBuf>,
        /// Jangan sertakan .INSTALL
        #[arg(long)]
        no_install_script: bool,
        /// Level kompresi zstd (1-22)
        #[arg(long, default_value_t = 19)]
        level: i32,
        #[arg(long)]
        json: bool,
    },
    /// Pasang paket hasil konversi via pacman -U
    Install {
        package: PathBuf,
        /// Jalankan pacman tanpa pkexec (butuh root)
        #[arg(long)]
        no_polkit: bool,
    },
    /// Hapus paket hasil konversi
    Uninstall {
        pkgname: String,
        #[arg(long)]
        no_polkit: bool,
    },
    /// Tampilkan riwayat konversi/instalasi
    History {
        #[arg(long)]
        json: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> deb_installer_core::Result<()> {
    match cli.command {
        Command::Info { file, json } => cmd_info(&file, json),
        Command::Files { file } => cmd_files(&file),
        Command::Deps {
            file,
            no_network,
            mapping,
            json,
        } => cmd_deps(&file, no_network, mapping.as_deref(), json),
        Command::Scripts { file } => cmd_scripts(&file),
        Command::Build {
            file,
            output,
            staging,
            no_install_script,
            level,
            json,
        } => cmd_build(&file, &output, staging, no_install_script, level, json),
        Command::Install { package, no_polkit } => cmd_install(&package, no_polkit),
        Command::Uninstall { pkgname, no_polkit } => cmd_uninstall(&pkgname, no_polkit),
        Command::History { json } => cmd_history(json),
    }
}

/// Apakah path adalah arsip tar yang didukung (fitur tambahan).
fn is_archive(path: &Path) -> bool {
    archive::compression_for(path).is_ok()
}

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn cmd_info(path: &Path, json: bool) -> deb_installer_core::Result<()> {
    if is_archive(path) {
        return archive_info(path, json);
    }
    let deb = DebPackage::open(path)?;
    let entries = deb.data_entries()?;
    let installed_size: u64 = entries
        .iter()
        .filter(|e| e.kind == FileKind::File)
        .map(|e| e.size)
        .sum();

    let info = serde_json::json!({
        "package": deb.control.package()?,
        "version": deb.control.version()?,
        "architecture": deb.control.architecture()?,
        "description": deb.control.synopsis(),
        "maintainer": deb.control.maintainer(),
        "homepage": deb.control.get("homepage").unwrap_or(""),
        "section": deb.control.get("section").unwrap_or(""),
        "file_size": deb.file_size,
        "installed_size": installed_size,
        "sha256": deb.sha256,
        "data_member": deb.data_member_name(),
        "file_count": entries.len(),
        "depends": deb.control.depends().iter().map(|r| r.to_debian_string()).collect::<Vec<_>>(),
        "scripts": deb.maintainer_scripts().keys().cloned().collect::<Vec<_>>(),
    });

    if json {
        println!("{}", serde_json::to_string_pretty(&info)?);
        return Ok(());
    }

    println!("Paket       : {}", deb.control.package()?);
    println!("Versi       : {}", deb.control.version()?);
    println!("Arsitektur  : {}", deb.control.architecture()?);
    println!("Deskripsi   : {}", deb.control.synopsis());
    println!("Pemelihara  : {}", deb.control.maintainer());
    if let Some(home) = deb.control.get("homepage") {
        println!("Homepage    : {home}");
    }
    println!("Ukuran file : {}", human_size(deb.file_size));
    println!("Terpasang   : {}", human_size(installed_size));
    println!("SHA256      : {}", deb.sha256);
    println!("Data member : {}", deb.data_member_name());
    println!("Jumlah berkas: {}", entries.len());
    let scripts = deb.maintainer_scripts();
    if scripts.is_empty() {
        println!("Script      : (tidak ada)");
    } else {
        println!(
            "Script      : {}",
            scripts.keys().cloned().collect::<Vec<_>>().join(", ")
        );
    }
    Ok(())
}

fn cmd_files(path: &Path) -> deb_installer_core::Result<()> {
    let deb = DebPackage::open(path)?;
    let entries = deb.data_entries()?;
    let mut sensitive = Vec::new();

    for entry in &entries {
        let normalized = deb_installer_core::normalize::normalize_path(&entry.path);
        if normalized != format!("/{}", entry.path) {
            println!("{:<60} -> {}", format!("/{}", entry.path), normalized);
        } else {
            println!("{normalized}");
        }
        if let Some(hit) = security::check_path(&normalized) {
            sensitive.push(hit);
        }
    }

    println!("\n{} entri", entries.len());
    if !sensitive.is_empty() {
        println!("\nPath sensitif:");
        for hit in sensitive {
            println!(
                "  [{}] {} — {}",
                match hit.level {
                    security::Sensitivity::Forbidden => "DITOLAK",
                    security::Sensitivity::Warning => "PERINGATAN",
                },
                hit.path,
                hit.reason
            );
        }
    }
    Ok(())
}

fn cmd_deps(
    path: &Path,
    no_network: bool,
    mapping: Option<&Path>,
    json: bool,
) -> deb_installer_core::Result<()> {
    let deb = DebPackage::open(path)?;
    let table = match mapping {
        Some(file) => {
            let text = std::fs::read_to_string(file)?;
            MappingTable::builtin().merged_with(&MappingTable::from_toml(&text)?)
        }
        None => MappingTable::builtin(),
    };

    let mut mapper = DepMapper::new(table, SystemPacman::new());
    mapper.allow_network = !no_network;
    let rels: Vec<_> = deb
        .control
        .pre_depends()
        .into_iter()
        .chain(deb.control.depends())
        .collect();
    let resolutions = mapper.resolve_all(&rels);

    if json {
        println!("{}", serde_json::to_string_pretty(&resolutions)?);
        return Ok(());
    }

    println!("{:<28} {:<24} {:<16} CATATAN", "DEBIAN", "ARCH", "STATUS");
    for res in &resolutions {
        let constraint = match (&res.constraint, res.alternatives.len()) {
            (Some(c), _) => format!(" {c}"),
            (None, n) if n > 1 => " (alternatif)".to_string(),
            _ => String::new(),
        };
        println!(
            "{:<28} {:<24} {:<16} {}",
            format!("{}{}", res.debian, constraint),
            res.arch_name.clone().unwrap_or_else(|| "-".into()),
            format!("{} ({})", res.status.label(), res.confidence.label()),
            res.note.clone().unwrap_or_default()
        );
    }

    let blocking = resolutions
        .iter()
        .filter(|r| r.status.is_blocking())
        .count();
    println!(
        "\n{} dependensi, {blocking} perlu perhatian",
        resolutions.len()
    );
    let _ = VersionOp::Eq;
    Ok(())
}

fn cmd_scripts(path: &Path) -> deb_installer_core::Result<()> {
    let deb = DebPackage::open(path)?;
    let scripts = deb.maintainer_scripts();
    if scripts.is_empty() {
        println!("Tidak ada script maintainer.");
        return Ok(());
    }
    for (name, body) in &scripts {
        println!("=== {name} ({} baris) ===", body.lines().count());
        for line in body.lines().take(40) {
            println!("  {line}");
        }
        if body.lines().count() > 40 {
            println!("  ... (dipotong)");
        }
    }

    let report = scripts::analyze(&scripts);
    println!("\nRisiko tertinggi: {}", report.max_risk().label());
    for finding in &report.findings {
        println!(
            "  [{}] {}:{} `{}` — {}",
            finding.risk.label(),
            finding.script,
            finding.line,
            finding.pattern,
            finding.advice
        );
    }
    if let Some(install) = scripts::build_install_script(&report) {
        println!("\n--- .INSTALL hasil terjemahan ---\n{install}");
    }
    Ok(())
}

fn cmd_build(
    path: &Path,
    output: &Path,
    staging: Option<PathBuf>,
    no_install_script: bool,
    level: i32,
    json: bool,
) -> deb_installer_core::Result<()> {
    if is_archive(path) {
        return archive_build(path, output, staging, level, json);
    }
    let deb = DebPackage::open(path)?;
    let table = MappingTable::builtin();
    let mut mapper = DepMapper::new(table, SystemPacman::new());
    let rels: Vec<_> = deb
        .control
        .pre_depends()
        .into_iter()
        .chain(deb.control.depends())
        .collect();
    let resolutions = mapper.resolve_all(&rels);

    // `_temp` menahan direktori sementara tetap hidup selama build.
    let mut _temp = None;
    let staging_dir = match staging {
        Some(dir) => dir,
        None => {
            let dir = tempfile::tempdir()?;
            let path = dir.path().join("stage");
            _temp = Some(dir);
            path
        }
    };

    let mut options = BuildOptions::new(staging_dir, output);
    options.include_install_script = !no_install_script;
    options.compression_level = level.clamp(1, 22);

    let builder = PackageBuilder::new(&deb, options);
    let result = builder.build(&resolutions)?;

    let mut history = History::load(&state::default_history_path()).unwrap_or_default();
    history.add(InstallRecord {
        id: state::record_id(deb.control.package()?, &deb.sha256),
        deb_name: deb.control.package()?.to_string(),
        deb_version: deb.control.version()?.to_string(),
        deb_path: std::fs::canonicalize(path)
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .to_string(),
        deb_sha256: deb.sha256.clone(),
        pkgname: result.pkgname.clone(),
        pkgver: result.full_version(),
        package_path: result.package_path.to_string_lossy().to_string(),
        converted_at: state::now_secs(),
        installed_at: None,
        uninstalled_at: None,
    });
    if let Err(e) = history.save(&state::default_history_path()) {
        eprintln!("peringatan: riwayat tidak tersimpan: {e}");
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    println!("Paket       : {}", result.pkgname);
    println!("Versi       : {}", result.full_version());
    println!("Arsitektur  : {}", result.arch);
    println!("Berkas      : {}", result.file_count);
    println!("Ukuran      : {}", human_size(result.installed_size));
    println!("Hasil       : {}", result.package_path.display());
    if let Some(script) = &result.install_script {
        println!("Script      : .INSTALL ({} baris)", script.lines().count());
    }
    for note in &result.notes {
        println!("Catatan     : {note}");
    }
    for warning in &result.warnings {
        println!("Peringatan  : {} — {}", warning.path, warning.reason);
    }
    Ok(())
}

fn cmd_install(package: &Path, no_polkit: bool) -> deb_installer_core::Result<()> {
    let mut installer = PacmanInstaller::new();
    installer.use_polkit = !no_polkit;

    let mut history = History::load(&state::default_history_path()).unwrap_or_default();
    let canonical = std::fs::canonicalize(package).unwrap_or_else(|_| package.to_path_buf());
    let record = history
        .records
        .iter()
        .find(|r| Path::new(&r.package_path) == canonical)
        .cloned();
    let pkgname = match &record {
        Some(r) => r.pkgname.clone(),
        None => parse_pkgname_from_filename(package).ok_or_else(|| {
            deb_installer_core::Error::Install(
                "nama paket tidak bisa dibaca dari nama berkas; konversi dulu lewat `build`".into(),
            )
        })?,
    };

    let outcome = installer.install(package, &mut |line| println!("{line}"))?;
    if outcome.success {
        if let Some(mut record) = record {
            record.installed_at = Some(state::now_secs());
            record.uninstalled_at = None;
            history.add(record);
            let _ = history.save(&state::default_history_path());
        }
        println!("\nBerhasil memasang {pkgname}");
        Ok(())
    } else {
        let msg = outcome
            .diagnosis_message
            .unwrap_or_else(|| "instalasi gagal".to_string());
        Err(deb_installer_core::Error::Install(msg))
    }
}

fn cmd_uninstall(pkgname: &str, no_polkit: bool) -> deb_installer_core::Result<()> {
    let mut installer = PacmanInstaller::new();
    installer.use_polkit = !no_polkit;
    let outcome = installer.uninstall(pkgname, &mut |line| println!("{line}"))?;
    if outcome.success {
        let mut history = History::load(&state::default_history_path()).unwrap_or_default();
        if history.mark_uninstalled(pkgname, state::now_secs()) {
            let _ = history.save(&state::default_history_path());
        }
        println!("\n{pkgname} dihapus");
        Ok(())
    } else {
        let msg = outcome
            .diagnosis_message
            .unwrap_or_else(|| "penghapusan gagal".to_string());
        Err(deb_installer_core::Error::Install(msg))
    }
}

fn cmd_history(json: bool) -> deb_installer_core::Result<()> {
    let history = History::load(&state::default_history_path())?;
    if json {
        println!("{}", serde_json::to_string_pretty(&history)?);
        return Ok(());
    }
    if history.records.is_empty() {
        println!("Belum ada riwayat.");
        return Ok(());
    }
    println!("{:<28} {:<20} {:<12} DEB", "PAKET", "VERSI", "STATUS");
    for record in &history.records {
        let status = if record.is_installed() {
            "terpasang"
        } else if record.uninstalled_at.is_some() {
            "dihapus"
        } else {
            "dikonversi"
        };
        println!(
            "{:<28} {:<20} {:<12} {}",
            record.pkgname, record.pkgver, status, record.deb_name
        );
    }
    Ok(())
}

/// `<pkgname>-<pkgver>-<pkgrel>-<arch>.pkg.tar.zst` → `pkgname`.
fn parse_pkgname_from_filename(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_string_lossy().to_string();
    let stem = name
        .strip_suffix(".pkg.tar.zst")
        .or_else(|| name.strip_suffix(".pkg.tar.xz"))?;
    let parts: Vec<&str> = stem.split('-').collect();
    if parts.len() < 4 {
        return None;
    }
    Some(parts[..parts.len() - 3].join("-"))
}

fn archive_info(path: &Path, json: bool) -> deb_installer_core::Result<()> {
    let pkg = ArchivePackage::open(path)?;
    let info = serde_json::json!({
        "kind": "archive",
        "package": pkg.name,
        "version": pkg.version,
        "layout": pkg.layout,
        "suggested_pkgname": pkg.suggested_pkgname(),
        "description": pkg.description,
        "file_size": pkg.file_size,
        "sha256": pkg.sha256,
        "file_count": pkg.entries.len(),
        "executables": pkg.executables,
    });
    if json {
        println!("{}", serde_json::to_string_pretty(&info)?);
        return Ok(());
    }
    println!("Arsip       : {}", path.display());
    println!("Nama        : {}", pkg.name);
    println!("Versi       : {}", pkg.version);
    println!("Tata letak  : {}", pkg.layout.label());
    if let ArchiveLayout::Portable { root } = &pkg.layout {
        match root {
            Some(root) => println!("Akar arsip  : {root}/"),
            None => println!("Akar arsip  : (berkas lepas)"),
        }
    }
    println!("Paket Arch  : {}", pkg.suggested_pkgname());
    if let Some(desc) = &pkg.description {
        println!("Deskripsi   : {desc}");
    }
    println!("Ukuran file : {}", human_size(pkg.file_size));
    println!("SHA256      : {}", pkg.sha256);
    println!("Jumlah entri: {}", pkg.entries.len());
    if !pkg.executables.is_empty() {
        println!("Binari      : {}", pkg.executables.join(", "));
    }
    Ok(())
}

fn archive_build(
    path: &Path,
    output: &Path,
    staging: Option<PathBuf>,
    level: i32,
    json: bool,
) -> deb_installer_core::Result<()> {
    let pkg = ArchivePackage::open(path)?;
    let mut _temp = None;
    let staging_dir = match staging {
        Some(dir) => dir,
        None => {
            let dir = tempfile::tempdir()?;
            let path = dir.path().join("stage");
            _temp = Some(dir);
            path
        }
    };

    let mut options = BuildOptions::new(staging_dir, output);
    options.compression_level = level.clamp(1, 22);
    let result = pkg.build(&options)?;

    let mut history = History::load(&state::default_history_path()).unwrap_or_default();
    history.add(InstallRecord {
        id: state::record_id(&pkg.name, &pkg.sha256),
        deb_name: pkg.name.clone(),
        deb_version: pkg.version.clone(),
        deb_path: std::fs::canonicalize(path)
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .to_string(),
        deb_sha256: pkg.sha256.clone(),
        pkgname: result.pkgname.clone(),
        pkgver: result.full_version(),
        package_path: result.package_path.to_string_lossy().to_string(),
        converted_at: state::now_secs(),
        installed_at: None,
        uninstalled_at: None,
    });
    if let Err(e) = history.save(&state::default_history_path()) {
        eprintln!("peringatan: riwayat tidak tersimpan: {e}");
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }
    println!("Paket       : {}", result.pkgname);
    println!("Versi       : {}", result.full_version());
    println!("Arsitektur  : {}", result.arch);
    println!("Berkas      : {}", result.file_count);
    println!("Ukuran      : {}", human_size(result.installed_size));
    println!("Hasil       : {}", result.package_path.display());
    for note in &result.notes {
        println!("Catatan     : {note}");
    }
    for warning in &result.warnings {
        println!("Peringatan  : {} — {}", warning.path, warning.reason);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_package_name_from_filename() {
        assert_eq!(
            parse_pkgname_from_filename(Path::new("code-deb-1.90.0-1-x86_64.pkg.tar.zst"))
                .as_deref(),
            Some("code-deb")
        );
        assert_eq!(
            parse_pkgname_from_filename(Path::new("my-app-deb-1.0-1-any.pkg.tar.zst")).as_deref(),
            Some("my-app-deb")
        );
        assert_eq!(parse_pkgname_from_filename(Path::new("bogus.zst")), None);
    }
}
