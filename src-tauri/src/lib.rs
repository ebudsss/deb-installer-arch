//! Backend Tauri: menjembatani UI dengan `deb-installer-core`.

use deb_installer_core::deb::{DebPackage, FileKind};
use deb_installer_core::deps::{
    Alternative, DepMapper, DepResolution, DepStatus, MappingTable, SystemPacman,
};
use deb_installer_core::install::{InstallOutcome, PacmanInstaller};
use deb_installer_core::pkgbuild::{self, BuildOptions, BuildResult, BuildStage, PackageBuilder};
use deb_installer_core::scripts::{self, Risk, ScriptReport};
use deb_installer_core::security::SensitivePath;
use deb_installer_core::state::{self, History, InstallRecord};
use deb_installer_core::{normalize, security};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager};

const MAX_ENTRIES: usize = 5000;

/// Ringkasan satu berkas di dalam paket, untuk tabel UI.
#[derive(Debug, Clone, Serialize)]
pub struct EntryView {
    pub path: String,
    pub original: String,
    pub kind: &'static str,
    pub size: u64,
    pub link_target: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DepSummary {
    pub total: usize,
    pub installed: usize,
    pub available: usize,
    pub aur: usize,
    pub missing: usize,
    pub ignored: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Analysis {
    pub path: String,
    pub package: String,
    pub version: String,
    pub arch: String,
    pub arch_mapped: String,
    pub suggested_pkgname: String,
    pub description: String,
    pub maintainer: String,
    pub homepage: String,
    pub file_size: u64,
    pub installed_size: u64,
    pub sha256: String,
    pub data_member: String,
    pub file_count: usize,
    pub entries: Vec<EntryView>,
    pub entries_truncated: bool,
    pub sensitive: Vec<SensitivePath>,
    pub deps: Vec<DepResolution>,
    pub dep_summary: DepSummary,
    pub scripts: ScriptReport,
    pub install_script: Option<String>,
    pub max_risk: Risk,
    pub alternatives: Vec<Alternative>,
    /// Konversi ditolak karena menyentuh path sistem yang dilindungi.
    pub blocked: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Progress {
    pub stage: String,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BuildRequest {
    pub path: String,
    /// Direktori keluaran; `None` = direktori cache aplikasi.
    #[serde(default)]
    pub output_dir: Option<String>,
    #[serde(default = "default_true")]
    pub include_install_script: bool,
    /// Level zstd. Bawaan 10: pada payload 295 MB, level 10 ≈ 3 detik untuk
    /// 118 MB, sedangkan level 19 ≈ 20 detik untuk 107 MB.
    #[serde(default = "default_level")]
    pub compression_level: i32,
    #[serde(default)]
    pub keep_staging: bool,
}

fn default_true() -> bool {
    true
}

fn default_level() -> i32 {
    10
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// `en` (bawaan) atau `id`.
    #[serde(default = "default_language")]
    pub language: String,
    /// `system`, `light`, atau `dark`.
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Direktori keluaran paket hasil konversi.
    #[serde(default)]
    pub output_dir: Option<String>,
    /// Simpan `.pkg.tar.zst` setelah instalasi berhasil.
    #[serde(default = "default_true")]
    pub keep_packages: bool,
    /// Query AUR saat menganalisis dependensi.
    #[serde(default = "default_true")]
    pub check_aur: bool,
    /// Isi `mapping.toml` tambahan milik pengguna.
    #[serde(default)]
    pub custom_mapping: String,
}

fn default_language() -> String {
    "en".to_string()
}

fn default_theme() -> String {
    "system".to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: default_language(),
            theme: default_theme(),
            output_dir: None,
            keep_packages: true,
            check_aur: true,
            custom_mapping: String::new(),
        }
    }
}

fn settings_path() -> PathBuf {
    state::data_dir().join("settings.json")
}

fn load_settings_inner() -> Settings {
    std::fs::read(settings_path())
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn mapping_table(settings: &Settings) -> deb_installer_core::Result<MappingTable> {
    let base = MappingTable::builtin();
    if settings.custom_mapping.trim().is_empty() {
        Ok(base)
    } else {
        Ok(base.merged_with(&MappingTable::from_toml(&settings.custom_mapping)?))
    }
}

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

fn kind_label(kind: FileKind) -> &'static str {
    match kind {
        FileKind::File => "file",
        FileKind::Directory => "dir",
        FileKind::Symlink => "symlink",
        FileKind::Hardlink => "hardlink",
    }
}

fn analyze_inner(path: &Path, settings: &Settings) -> Result<Analysis, String> {
    let deb = DebPackage::open(path).map_err(err)?;
    let entries = deb.data_entries().map_err(err)?;

    let mut views = Vec::with_capacity(entries.len().min(MAX_ENTRIES));
    let mut sensitive = Vec::new();
    let mut forbidden = false;
    let mut installed_size = 0u64;
    let mut file_count = 0usize;

    for entry in &entries {
        if entry.kind == FileKind::File {
            installed_size += entry.size;
            file_count += 1;
        }
        let normalized = normalize::normalize_path(&entry.path);
        if let Some(hit) = security::check_path(&normalized) {
            if hit.level == security::Sensitivity::Forbidden {
                forbidden = true;
            }
            sensitive.push(hit);
        }
        if views.len() < MAX_ENTRIES {
            views.push(EntryView {
                path: normalized,
                original: entry.path.clone(),
                kind: kind_label(entry.kind),
                size: entry.size,
                link_target: entry.link_target.clone(),
            });
        }
    }

    let table = mapping_table(settings).map_err(err)?;
    let mut mapper = DepMapper::new(table, SystemPacman::new());
    mapper.allow_network = settings.check_aur;

    let relationships: Vec<_> = deb
        .control
        .pre_depends()
        .into_iter()
        .chain(deb.control.depends())
        .collect();
    let deps = mapper.resolve_all(&relationships);

    let mut summary = DepSummary {
        total: deps.len(),
        ..DepSummary::default()
    };
    for dep in &deps {
        match dep.status {
            DepStatus::Installed => summary.installed += 1,
            DepStatus::Available => summary.available += 1,
            DepStatus::AurOnly => summary.aur += 1,
            DepStatus::Missing => summary.missing += 1,
            DepStatus::Ignored => summary.ignored += 1,
        }
    }

    let package = deb.control.package().map_err(err)?.to_string();
    let alternatives = mapper.suggest_alternatives(&package);

    let scripts = scripts::analyze(&deb.maintainer_scripts());
    let install_script = scripts::build_install_script(&scripts);
    let max_risk = scripts.max_risk();

    // Hanya path sistem yang dilindungi yang memblokir konversi. Script
    // berisiko tinggi tidak dieksekusi sama sekali, tetapi UI mewajibkan
    // persetujuan eksplisit sebelum instalasi.
    let blocked = forbidden;

    Ok(Analysis {
        path: std::fs::canonicalize(path)
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .to_string(),
        package: package.clone(),
        version: deb.control.version().map_err(err)?.to_string(),
        arch: deb.control.architecture().map_err(err)?.to_string(),
        arch_mapped: pkgbuild::convert_arch(deb.control.architecture().map_err(err)?),
        suggested_pkgname: pkgbuild::arch_package_name(&package),
        description: deb.control.description().to_string(),
        maintainer: deb.control.maintainer().to_string(),
        homepage: deb.control.get("homepage").unwrap_or("").to_string(),
        file_size: deb.file_size,
        installed_size,
        sha256: deb.sha256.clone(),
        data_member: deb.data_member_name().to_string(),
        file_count,
        entries_truncated: entries.len() > views.len(),
        entries: views,
        sensitive,
        deps,
        dep_summary: summary,
        scripts,
        install_script,
        max_risk,
        alternatives,
        blocked,
    })
}

/// Path `.deb` yang diberikan lewat argumen CLI / asosiasi berkas.
#[tauri::command]
fn initial_file() -> Option<String> {
    std::env::args()
        .skip(1)
        .find(|arg| arg.to_lowercase().ends_with(".deb"))
}

#[tauri::command]
async fn analyze(path: String) -> Result<Analysis, String> {
    let settings = load_settings_inner();
    tauri::async_runtime::spawn_blocking(move || analyze_inner(Path::new(&path), &settings))
        .await
        .map_err(|e| format!("tugas analisis gagal: {e}"))?
}

#[tauri::command]
async fn build_package(app: AppHandle, request: BuildRequest) -> Result<BuildResult, String> {
    let settings = load_settings_inner();
    tauri::async_runtime::spawn_blocking(move || {
        let emit = |stage: &str, message: String| {
            let _ = app.emit(
                "progress",
                Progress {
                    stage: stage.to_string(),
                    message,
                },
            );
        };

        emit("analyze", "Membaca paket .deb".into());
        let deb = DebPackage::open(&request.path).map_err(err)?;

        let table = mapping_table(&settings).map_err(err)?;
        let mut mapper = DepMapper::new(table, SystemPacman::new());
        mapper.allow_network = settings.check_aur;
        let relationships: Vec<_> = deb
            .control
            .pre_depends()
            .into_iter()
            .chain(deb.control.depends())
            .collect();
        emit("deps", "Memetakan dependensi".into());
        let deps = mapper.resolve_all(&relationships);

        let output_dir = request
            .output_dir
            .clone()
            .map(PathBuf::from)
            .or_else(|| settings.output_dir.clone().map(PathBuf::from))
            .unwrap_or_else(state::default_cache_dir);
        std::fs::create_dir_all(&output_dir).map_err(err)?;

        // TempDir menghapus staging saat drop; `keep_staging` menyimpannya
        // di samping paket hasil konversi untuk inspeksi manual.
        let temp_dir = if request.keep_staging {
            None
        } else {
            Some(tempfile::tempdir().map_err(err)?)
        };
        let staging_root = match &temp_dir {
            Some(dir) => dir.path().to_path_buf(),
            None => output_dir.join(format!("{}-staging", deb.control.package().map_err(err)?)),
        };
        let staging = staging_root.join("stage");
        if staging.exists() {
            std::fs::remove_dir_all(&staging).map_err(err)?;
        }

        let mut options = BuildOptions::new(staging, &output_dir);
        options.include_install_script = request.include_install_script;
        options.compression_level = request.compression_level.clamp(1, 22);

        emit("convert", "Mengekstrak dan menormalisasi berkas".into());
        let builder = PackageBuilder::new(&deb, options);
        let emit_stage = |stage: BuildStage| {
            let _ = app.emit(
                "progress",
                Progress {
                    stage: "convert".to_string(),
                    message: stage.message(),
                },
            );
        };
        let result = builder
            .build_with_progress(&deps, &mut |stage| emit_stage(stage))
            .map_err(err)?;

        emit("done", format!("Paket siap: {}", result.package_path.display()));

        let mut history = History::load(&state::default_history_path()).unwrap_or_default();
        history.add(InstallRecord {
            id: state::record_id(deb.control.package().map_err(err)?, &deb.sha256),
            deb_name: deb.control.package().map_err(err)?.to_string(),
            deb_version: deb.control.version().map_err(err)?.to_string(),
            deb_path: std::fs::canonicalize(&request.path)
                .unwrap_or_else(|_| PathBuf::from(&request.path))
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

        Ok(result)
    })
    .await
    .map_err(|e| format!("tugas konversi gagal: {e}"))?
}

#[tauri::command]
async fn install_package(
    app: AppHandle,
    package_path: String,
    keep_package: Option<bool>,
) -> Result<InstallOutcome, String> {
    let keep_package = keep_package.unwrap_or(true);
    tauri::async_runtime::spawn_blocking(move || {
        let installer = PacmanInstaller::new();
        let emit_app = app.clone();
        let outcome = installer
            .install(Path::new(&package_path), &mut |line| {
                let _ = emit_app.emit("install-log", line.to_string());
            })
            .map_err(err)?;

        if outcome.success {
            let mut history = History::load(&state::default_history_path()).unwrap_or_default();
            if let Some(record) = history
                .records
                .iter_mut()
                .find(|r| Path::new(&r.package_path) == Path::new(&package_path))
            {
                record.installed_at = Some(state::now_secs());
                record.uninstalled_at = None;
            }
            let _ = history.save(&state::default_history_path());
        }

        if outcome.success && !keep_package {
            let _ = std::fs::remove_file(&package_path);
        }

        let _ = app.emit(
            "install-done",
            serde_json::json!({
                "success": outcome.success,
                "message": outcome.diagnosis_message,
            }),
        );
        Ok(outcome)
    })
    .await
    .map_err(|e| format!("tugas instalasi gagal: {e}"))?
}

#[tauri::command]
async fn uninstall_package(app: AppHandle, pkgname: String) -> Result<InstallOutcome, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let installer = PacmanInstaller::new();
        let emit_app = app.clone();
        let outcome = installer
            .uninstall(&pkgname, &mut |line| {
                let _ = emit_app.emit("install-log", line.to_string());
            })
            .map_err(err)?;

        if outcome.success {
            let mut history = History::load(&state::default_history_path()).unwrap_or_default();
            if history.mark_uninstalled(&pkgname, state::now_secs()) {
                let _ = history.save(&state::default_history_path());
            }
        }
        Ok(outcome)
    })
    .await
    .map_err(|e| format!("tugas penghapusan gagal: {e}"))?
}

#[tauri::command]
fn list_installed() -> Result<Vec<InstallRecord>, String> {
    let history = History::load(&state::default_history_path()).map_err(err)?;
    let mut records = history.records.clone();
    records.sort_by_key(|record| std::cmp::Reverse(record.converted_at));
    Ok(records)
}

#[tauri::command]
fn get_settings() -> Settings {
    load_settings_inner()
}

#[tauri::command]
fn save_settings(settings: Settings) -> Result<(), String> {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(err)?;
    }
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&settings).map_err(err)?,
    )
    .map_err(err)
}

/// Menyegarkan database `pacman -F` agar pemetaan lewat soname bekerja.
#[tauri::command]
async fn refresh_file_db(app: AppHandle) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut child = std::process::Command::new("pkexec")
            .args(["pacman", "-Fy"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("tidak bisa menjalankan pkexec: {e}"))?;

        use std::io::{BufRead, BufReader};
        if let Some(stderr) = child.stderr.take() {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                let _ = app.emit("install-log", line);
            }
        }
        let status = child.wait().map_err(err)?;
        Ok(status.success())
    })
    .await
    .map_err(|e| format!("tugas refresh gagal: {e}"))?
}

#[tauri::command]
fn delete_record(app: AppHandle, pkgname: String) -> Result<(), String> {
    let mut history = History::load(&state::default_history_path()).map_err(err)?;
    history.records.retain(|r| r.pkgname != pkgname);
    history.save(&state::default_history_path()).map_err(err)?;
    let _ = app;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Pastikan direktori data ada sejak awal.
            let _ = std::fs::create_dir_all(state::data_dir());
            let _ = app.get_webview_window("main");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            initial_file,
            analyze,
            build_package,
            install_package,
            uninstall_package,
            list_installed,
            get_settings,
            save_settings,
            refresh_file_db,
            delete_record
        ])
        .run(tauri::generate_context!())
        .expect("gagal menjalankan DebInstaller");
}
