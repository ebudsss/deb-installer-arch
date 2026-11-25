//! Menjalankan `pacman -U` / `pacman -Rns` dengan eskalasi polkit (`pkexec`).
//!
//! Hanya langkah ini yang berjalan sebagai root; konversi tetap sebagai user.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;

/// Penyebab kegagalan yang bisa dikenali dari log pacman.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Diagnosis {
    /// Berkas akan menimpa milik paket lain.
    FileConflict(Vec<String>),
    /// Dependensi belum tersedia.
    MissingDependencies(Vec<String>),
    /// `db.lck` masih ada; pacman lain sedang berjalan.
    DatabaseLocked,
    /// Paket rusak atau tidak valid.
    InvalidPackage,
    /// Autentikasi polkit dibatalkan atau gagal.
    NotAuthorized,
    /// Perintah berjalan tanpa hak root (pkexec tidak dipakai atau gagal).
    NotRoot,
    /// Paket target tidak ditemukan di repo/database.
    TargetNotFound,
    /// Gagal verifikasi tanda tangan.
    SignatureError,
    Unknown,
}

impl Diagnosis {
    pub fn message(&self) -> String {
        match self {
            Diagnosis::FileConflict(files) => format!(
                "Berkas berikut sudah dimiliki paket lain: {}. Gunakan `pacman -U --overwrite` hanya setelah meninjau daftar tersebut.",
                files.join(", ")
            ),
            Diagnosis::MissingDependencies(deps) => format!(
                "Dependensi berikut tidak bisa dipasang otomatis: {}. Pastikan repo pacman tersinkron (`pacman -Sy`), lalu ulangi.",
                deps.join(", ")
            ),
            Diagnosis::DatabaseLocked => {
                "Database pacman terkunci (proses lain sedang berjalan). Tutup pamac/Octopi atau tunggu hingga selesai.".to_string()
            }
            Diagnosis::InvalidPackage => {
                "Paket hasil konversi tidak valid atau rusak. Coba konversi ulang.".to_string()
            }
            Diagnosis::NotAuthorized => {
                "Autentikasi dibatalkan atau ditolak. Instalasi memerlukan izin administrator.".to_string()
            }
            Diagnosis::NotRoot => {
                "pacman dijalankan tanpa hak administrator. Pastikan pkexec dan agen polkit terpasang, lalu coba lagi.".to_string()
            }
            Diagnosis::TargetNotFound => {
                "Paket tidak ditemukan oleh pacman. Pastikan berkas .pkg.tar.zst masih ada.".to_string()
            }
            Diagnosis::SignatureError => {
                "Verifikasi tanda tangan gagal. Periksa konfigurasi SigLevel pacman.".to_string()
            }
            Diagnosis::Unknown => {
                "pacman gagal tanpa pola yang dikenali; periksa log di atas.".to_string()
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallOutcome {
    pub success: bool,
    pub exit_code: Option<i32>,
    /// Baris log lengkap (stdout+stderr).
    pub log: Vec<String>,
    pub diagnosis: Option<Diagnosis>,
    pub diagnosis_message: Option<String>,
}

impl InstallOutcome {
    fn from_log(success: bool, exit_code: Option<i32>, log: Vec<String>) -> Self {
        let diagnosis = if success { None } else { Some(diagnose(&log)) };
        let diagnosis_message = diagnosis.as_ref().map(Diagnosis::message);
        Self {
            success,
            exit_code,
            log,
            diagnosis,
            diagnosis_message,
        }
    }
}

/// Menganalisis log pacman untuk memberi saran yang bisa ditindaklanjuti.
pub fn diagnose(log: &[String]) -> Diagnosis {
    let text = log.join("\n");

    if text.contains("unable to lock database") {
        return Diagnosis::DatabaseLocked;
    }
    if text.contains("unless you are root") {
        return Diagnosis::NotRoot;
    }
    if text.contains("Not authorized")
        || text.contains("Authorization failed")
        || text.contains("Error executing command as another user")
        || text.contains("Request dismissed")
    {
        return Diagnosis::NotAuthorized;
    }
    if text.contains("signature") && (text.contains("is invalid") || text.contains("unknown trust"))
    {
        return Diagnosis::SignatureError;
    }
    if text.contains("invalid or corrupted package")
        || text.contains("failed to commit transaction (invalid")
        || text.contains("could not open file")
    {
        return Diagnosis::InvalidPackage;
    }
    if text.contains("target not found") || text.contains("could not find or read package") {
        return Diagnosis::TargetNotFound;
    }
    if text.contains("exists in filesystem") {
        let mut files = Vec::new();
        for line in log {
            let Some(idx) = line.find(" exists in filesystem") else {
                continue;
            };
            let prefix = &line[..idx];
            let path = prefix
                .rsplit_once(": ")
                .map(|(_, p)| p)
                .unwrap_or(prefix)
                .trim();
            if !path.is_empty() && !files.iter().any(|f| f == path) {
                files.push(path.to_string());
            }
        }
        return Diagnosis::FileConflict(files);
    }
    if text.contains("unable to satisfy dependency")
        || text.contains("could not satisfy dependencies")
        || text.contains("missing dependencies")
    {
        let mut deps = Vec::new();
        for line in log {
            if let Some(idx) = line.find("unable to satisfy dependency") {
                let rest = &line[idx..];
                if let Some(start) = rest.find('\'') {
                    if let Some(end) = rest[start + 1..].find('\'') {
                        let dep = &rest[start + 1..start + 1 + end];
                        if !deps.iter().any(|d: &String| d == dep) {
                            deps.push(dep.to_string());
                        }
                    }
                }
            }
        }
        return Diagnosis::MissingDependencies(deps);
    }
    Diagnosis::Unknown
}

/// Nama paket Arch yang aman dirangkai ke perintah shell. Nama datang dari
/// `.PKGINFO` di dalam paket, sehingga harus divalidasi sebelum dipakai.
fn is_valid_package_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '@' | '.' | '_' | '+' | '-'))
}

/// Mengutip satu argumen untuk shell POSIX.
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// Dependensi yang dideklarasikan `.PKGINFO` di dalam berkas paket.
///
/// Dibaca dari paketnya sendiri, bukan dari UI, agar pemasangan tetap benar
/// walau berkas dibuka langsung dari cache.
pub fn package_dependencies(package: &Path) -> Vec<String> {
    let mut deps = Vec::new();
    let Ok(text) = read_pkginfo(package) else {
        return deps;
    };
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("depend = ") {
            let name = rest.trim();
            if !name.is_empty() {
                deps.push(name.to_string());
            }
        }
    }
    deps.sort();
    deps.dedup();
    deps
}

/// Membaca `.PKGINFO` dari paket `.pkg.tar.zst` / `.pkg.tar.xz`.
fn read_pkginfo(package: &Path) -> Result<String> {
    let file = File::open(package)
        .map_err(|e| Error::Install(format!("tidak bisa membuka paket: {e}")))?;
    let reader: Box<dyn Read> = if package.to_string_lossy().ends_with(".pkg.tar.xz") {
        Box::new(xz2::read::XzDecoder::new(file))
    } else {
        Box::new(
            zstd::stream::read::Decoder::new(file)
                .map_err(|e| Error::Install(format!("paket bukan zstd yang valid: {e}")))?,
        )
    };
    let mut archive = tar::Archive::new(reader);
    let entries = archive
        .entries()
        .map_err(|e| Error::Install(format!("tidak bisa membaca arsip paket: {e}")))?;
    for entry in entries {
        let mut entry = entry.map_err(|e| Error::Install(format!("entri paket rusak: {e}")))?;
        let is_pkginfo = entry
            .path()
            .map(|path| path.to_string_lossy().trim_start_matches("./") == ".PKGINFO")
            .unwrap_or(false);
        if is_pkginfo {
            let mut text = String::new();
            entry
                .read_to_string(&mut text)
                .map_err(|e| Error::Install(format!(".PKGINFO tidak terbaca: {e}")))?;
            return Ok(text);
        }
    }
    Ok(String::new())
}

/// Pemasang paket berbasis pacman.
#[derive(Debug, Clone)]
pub struct PacmanInstaller {
    pub pacman: String,
    pub pkexec: String,
    /// Bila `false`, perintah dijalankan langsung (tanpa eskalasi). Berguna
    /// untuk pengujian dan untuk dijalankan dari root.
    pub use_polkit: bool,
    /// Batas waktu tanpa keluaran sama sekali sebelum proses dianggap macet.
    /// Tanpa agen polkit, `pkexec` menunggu selamanya.
    pub idle_timeout: std::time::Duration,
}

impl Default for PacmanInstaller {
    fn default() -> Self {
        Self {
            pacman: "pacman".to_string(),
            pkexec: "pkexec".to_string(),
            use_polkit: true,
            idle_timeout: std::time::Duration::from_secs(90),
        }
    }
}

impl PacmanInstaller {
    pub fn new() -> Self {
        let mut installer = Self::default();
        // Sudah root: eskalasi hanya akan meminta autentikasi yang tidak perlu.
        if unsafe { libc::geteuid() } == 0 {
            installer.use_polkit = false;
        }
        installer
    }

    /// Memeriksa prasyarat sebelum meminta autentikasi.
    pub fn preflight(&self, package: &Path) -> Result<()> {
        if !package.exists() {
            return Err(Error::Install(format!(
                "paket tidak ditemukan: {}",
                package.display()
            )));
        }
        let name = package.to_string_lossy();
        if !name.ends_with(".pkg.tar.zst") && !name.ends_with(".pkg.tar.xz") {
            return Err(Error::Install(
                "berkas bukan paket pacman (.pkg.tar.zst)".into(),
            ));
        }
        if Path::new("/var/lib/pacman/db.lck").exists() {
            return Err(Error::Install(Diagnosis::DatabaseLocked.message()));
        }
        Ok(())
    }

    /// Memasang paket hasil konversi, meneruskan setiap baris log ke `on_line`.
    ///
    /// Dependensi yang belum terpenuhi dipasang lebih dulu sebagai dependensi
    /// (`--asdeps`) dalam satu panggilan berhak istimewa, sehingga pengguna
    /// hanya diautentikasi sekali dan `pacman -U` tidak gagal karena repo
    /// belum lengkap.
    pub fn install(&self, package: &Path, on_line: &mut dyn FnMut(&str)) -> Result<InstallOutcome> {
        self.preflight(package)?;

        let deps = package_dependencies(package);
        let missing: Vec<String> = self
            .missing_dependencies(&deps)
            .into_iter()
            .filter(|name| is_valid_package_name(name))
            .collect();

        if missing.is_empty() {
            return self.run(&self.install_args(package), on_line);
        }

        on_line(&format!(
            "Dependensi belum terpasang: {}. Memasang otomatis.",
            missing.join(", ")
        ));
        let names = missing
            .iter()
            .map(|name| shell_quote(name))
            .collect::<Vec<_>>()
            .join(" ");
        let script = format!(
            "{} -S --asdeps --noconfirm --noprogressbar {names} && {}",
            shell_quote(&self.pacman),
            self.install_command(package)
        );
        self.run_script(&script, on_line)
    }

    fn install_args(&self, package: &Path) -> Vec<String> {
        vec![
            "-U".to_string(),
            "--noconfirm".to_string(),
            "--noprogressbar".to_string(),
            package.to_string_lossy().to_string(),
        ]
    }

    /// Fragmen perintah `pacman -U` untuk dirangkai di dalam skrip shell.
    fn install_command(&self, package: &Path) -> String {
        format!(
            "{} -U --noconfirm --noprogressbar {}",
            shell_quote(&self.pacman),
            shell_quote(&package.to_string_lossy())
        )
    }

    /// Dependensi paket yang belum terpenuhi di sistem (`pacman -T`).
    pub fn missing_dependencies(&self, deps: &[String]) -> Vec<String> {
        let names: Vec<&String> = deps
            .iter()
            .filter(|name| is_valid_package_name(name))
            .collect();
        if names.is_empty() {
            return Vec::new();
        }
        let mut args = vec!["-T".to_string()];
        args.extend(names.iter().map(|name| (*name).clone()));
        match Command::new(&self.pacman).args(&args).output() {
            // Exit 0 berarti semua dependensi sudah terpenuhi.
            Ok(out) if out.status.success() => Vec::new(),
            Ok(out) => String::from_utf8_lossy(&out.stdout)
                .lines()
                .map(|line| line.trim().to_string())
                .filter(|line| !line.is_empty())
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Menghapus paket beserta dependensi yang tidak lagi dipakai.
    pub fn uninstall(
        &self,
        pkgname: &str,
        on_line: &mut dyn FnMut(&str),
    ) -> Result<InstallOutcome> {
        if pkgname.trim().is_empty() {
            return Err(Error::Install("nama paket kosong".into()));
        }
        let args = vec![
            "-Rns".to_string(),
            "--noconfirm".to_string(),
            "--noprogressbar".to_string(),
            pkgname.to_string(),
        ];
        self.run(&args, on_line)
    }

    /// Apakah paket terpasang menurut database pacman.
    pub fn is_installed(&self, pkgname: &str) -> bool {
        Command::new(&self.pacman)
            .args(["-Qq", pkgname])
            .output()
            .map(|o| {
                o.status.success()
                    && String::from_utf8_lossy(&o.stdout)
                        .lines()
                        .any(|l| l.trim() == pkgname)
            })
            .unwrap_or(false)
    }

    /// Perintah lengkap yang akan dijalankan (untuk ditampilkan di UI/log).
    pub fn describe(&self, args: &[String]) -> String {
        if self.use_polkit {
            format!("{} {} {}", self.pkexec, self.pacman, args.join(" "))
        } else {
            format!("{} {}", self.pacman, args.join(" "))
        }
    }

    fn run(&self, args: &[String], on_line: &mut dyn FnMut(&str)) -> Result<InstallOutcome> {
        let (program, full_args) = if self.use_polkit {
            let mut full = vec![self.pacman.clone()];
            full.extend(args.iter().cloned());
            (self.pkexec.clone(), full)
        } else {
            (self.pacman.clone(), args.to_vec())
        };

        on_line(&format!("$ {}", self.describe(args)));
        self.spawn_and_collect(&program, &full_args, on_line)
    }

    /// Menjalankan perintah melalui shell, dengan eskalasi bila perlu.
    fn run_script(&self, script: &str, on_line: &mut dyn FnMut(&str)) -> Result<InstallOutcome> {
        let (program, args) = if self.use_polkit {
            (
                self.pkexec.clone(),
                vec!["sh".to_string(), "-c".to_string(), script.to_string()],
            )
        } else {
            ("sh".to_string(), vec!["-c".to_string(), script.to_string()])
        };
        let shown = if self.use_polkit {
            format!("{} {}", self.pkexec, script)
        } else {
            script.to_string()
        };
        on_line(&format!("$ {shown}"));
        self.spawn_and_collect(&program, &args, on_line)
    }

    fn spawn_and_collect(
        &self,
        program: &str,
        full_args: &[String],
        on_line: &mut dyn FnMut(&str),
    ) -> Result<InstallOutcome> {
        let mut child = Command::new(program)
            .args(full_args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                Error::Install(format!(
                    "tidak bisa menjalankan `{program}`: {e}. Pastikan polkit/pkexec terpasang."
                ))
            })?;

        let (tx, rx) = mpsc::channel::<String>();
        let stdout = child.stdout.take().expect("stdout piped");
        let stderr = child.stderr.take().expect("stderr piped");

        let tx_out = tx.clone();
        let handle_out = std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let _ = tx_out.send(line);
            }
        });
        let tx_err = tx.clone();
        let handle_err = std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                let _ = tx_err.send(line);
            }
        });
        drop(tx);

        let mut log = Vec::new();
        let mut timed_out = false;
        let idle = self.idle_timeout;
        loop {
            match rx.recv_timeout(idle) {
                Ok(line) => {
                    on_line(&line);
                    log.push(line);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    timed_out = true;
                    let _ = child.kill();
                    break;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }

        let _ = handle_out.join();
        let _ = handle_err.join();
        let status = child.wait()?;

        if timed_out {
            let message = if self.use_polkit {
                "Tidak ada respons dari polkit. Pastikan agen autentikasi (mis. gnome-shell, polkit-kde-authentication-agent-1, atau xfce-polkit) berjalan di sesi ini."
            } else {
                "pacman tidak merespons; proses dihentikan."
            };
            on_line(message);
            log.push(message.to_string());
            let mut outcome = InstallOutcome::from_log(false, status.code(), log);
            outcome.diagnosis = Some(Diagnosis::NotAuthorized);
            outcome.diagnosis_message = Some(message.to_string());
            return Ok(outcome);
        }

        Ok(InstallOutcome::from_log(
            status.success(),
            status.code(),
            log,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    /// Membuat paket `.pkg.tar.zst` minimal dengan isi `.PKGINFO` tertentu.
    fn make_package(dir: &Path, pkginfo: &str) -> std::path::PathBuf {
        let path = dir.join("demo-deb-1.0-1-x86_64.pkg.tar.zst");
        let file = std::fs::File::create(&path).unwrap();
        let encoder = zstd::stream::write::Encoder::new(file, 3).unwrap();
        let mut builder = tar::Builder::new(encoder.auto_finish());
        let mut header = tar::Header::new_gnu();
        header.set_size(pkginfo.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, ".PKGINFO", pkginfo.as_bytes())
            .unwrap();
        builder.finish().unwrap();
        path
    }

    /// Skrip pacman palsu yang mencatat setiap panggilan ke `log`.
    fn fake_pacman(dir: &Path, missing: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let log = dir.join("calls.log");
        let script = dir.join("pacman");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\n\
echo \"$@\" >> {log}\n\
case \"$1\" in\n\
  -T) echo \"{missing}\"; exit 1;;\n\
  -S) echo \"memasang dependensi\"; exit 0;;\n\
  -U) echo \"memasang paket\"; exit 0;;\n\
esac\nexit 0\n",
                log = log.display()
            ),
        )
        .unwrap();
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
        std::fs::set_permissions(&script, perms).unwrap();
        (script, log)
    }

    #[test]
    fn reads_dependencies_from_package() {
        let dir = tempfile::tempdir().unwrap();
        let package = make_package(
            dir.path(),
            "pkgname = demo-deb\ndepend = gtk3\ndepend = libfoo\n",
        );
        assert_eq!(
            package_dependencies(&package),
            vec!["gtk3".to_string(), "libfoo".to_string()]
        );
    }

    #[test]
    fn quotes_package_names_for_shell() {
        assert!(is_valid_package_name("gtk3"));
        assert!(is_valid_package_name("lib32-gtk3"));
        assert!(!is_valid_package_name("gtk3; rm -rf /"));
        assert!(!is_valid_package_name(""));
        assert_eq!(shell_quote("a b"), "'a b'");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
    }

    #[test]
    fn installs_missing_dependencies_before_package() {
        let dir = tempfile::tempdir().unwrap();
        let package = make_package(dir.path(), "pkgname = demo-deb\ndepend = libmissing\n");
        let (pacman, log) = fake_pacman(dir.path(), "libmissing");
        let installer = PacmanInstaller {
            pacman: pacman.to_string_lossy().to_string(),
            use_polkit: false,
            ..PacmanInstaller::default()
        };

        let mut lines = Vec::new();
        let outcome = installer
            .install(&package, &mut |line| lines.push(line.to_string()))
            .unwrap();
        assert!(outcome.success);

        let calls = std::fs::read_to_string(&log).unwrap();
        let calls: Vec<&str> = calls.lines().collect();
        // Cek ketersediaan, pasang dependensi, baru pasang paketnya.
        assert_eq!(calls[0], "-T libmissing");
        assert_eq!(
            calls[1],
            "-S --asdeps --noconfirm --noprogressbar libmissing"
        );
        assert!(calls[2].starts_with("-U --noconfirm --noprogressbar "));
        assert!(lines
            .iter()
            .any(|line| line.contains("Dependensi belum terpasang: libmissing")));
    }

    #[test]
    fn skips_dependency_step_when_nothing_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let package = make_package(dir.path(), "pkgname = demo-deb\ndepend = gtk3\n");
        let log = dir.path().join("calls.log");
        let script = dir.path().join("pacman");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\necho \"$@\" >> {}\n[ \"$1\" = \"-T\" ] && exit 0\nexit 0\n",
                log.display()
            ),
        )
        .unwrap();
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
        std::fs::set_permissions(&script, perms).unwrap();

        let installer = PacmanInstaller {
            pacman: script.to_string_lossy().to_string(),
            use_polkit: false,
            ..PacmanInstaller::default()
        };
        installer.install(&package, &mut |_| {}).unwrap();

        let calls = std::fs::read_to_string(&log).unwrap();
        assert_eq!(calls.lines().count(), 2);
        assert!(calls.contains("-T gtk3"));
        assert!(calls.contains("-U --noconfirm --noprogressbar"));
    }

    #[test]
    fn diagnoses_file_conflict() {
        let log = lines(&[
            "error: failed to commit transaction (conflicting files)",
            "vscode-deb: /usr/bin/code exists in filesystem (owned by code)",
            "Errors occurred, no packages were upgraded.",
        ]);
        match diagnose(&log) {
            Diagnosis::FileConflict(files) => assert_eq!(files, vec!["/usr/bin/code"]),
            other => panic!("diagnosis salah: {other:?}"),
        }
    }

    #[test]
    fn diagnoses_missing_dependency() {
        let log = lines(&[
            "error: failed to prepare transaction (could not satisfy dependencies)",
            ":: installing foo-deb (1.0-1) breaks dependency 'bar' required by baz",
            "error: unable to satisfy dependency 'libssl.so.1.1' required by foo-deb",
        ]);
        match diagnose(&log) {
            Diagnosis::MissingDependencies(deps) => {
                assert!(deps.contains(&"libssl.so.1.1".to_string()))
            }
            other => panic!("diagnosis salah: {other:?}"),
        }
    }

    #[test]
    fn diagnoses_lock_and_auth_and_signature() {
        assert_eq!(
            diagnose(&lines(&["error: unable to lock database: File exists"])),
            Diagnosis::DatabaseLocked
        );
        assert_eq!(
            diagnose(&lines(&[
                "Error executing command as another user: Not authorized"
            ])),
            Diagnosis::NotAuthorized
        );
        assert_eq!(
            diagnose(&lines(&[
                "error: you cannot perform this operation unless you are root."
            ])),
            Diagnosis::NotRoot
        );
        assert_eq!(
            diagnose(&lines(&["error: signature from \"X\" is invalid"])),
            Diagnosis::SignatureError
        );
        assert_eq!(
            diagnose(&lines(&["error: 'foo-deb': target not found"])),
            Diagnosis::TargetNotFound
        );
        assert_eq!(
            diagnose(&lines(&["error: invalid or corrupted package (checksum)"])),
            Diagnosis::InvalidPackage
        );
        assert_eq!(
            diagnose(&lines(&["everything exploded"])),
            Diagnosis::Unknown
        );
    }

    #[test]
    fn outcome_populates_message() {
        let outcome = InstallOutcome::from_log(
            false,
            Some(1),
            lines(&["error: unable to lock database: File exists"]),
        );
        assert!(!outcome.success);
        assert_eq!(outcome.diagnosis, Some(Diagnosis::DatabaseLocked));
        assert!(outcome.diagnosis_message.unwrap().contains("terkunci"));
    }

    #[test]
    fn preflight_rejects_non_package() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("x.deb");
        std::fs::write(&file, b"x").unwrap();
        let installer = PacmanInstaller::new();
        let err = installer.preflight(&file).unwrap_err();
        assert!(matches!(err, Error::Install(_)));
        assert!(installer
            .preflight(&dir.path().join("missing.pkg.tar.zst"))
            .is_err());
    }

    #[test]
    fn idle_timeout_reports_missing_polkit_agent() {
        // `sleep` tidak pernah menghasilkan keluaran; pemasang harus
        // menghentikannya dan melaporkan masalah autentikasi.
        let dir = tempfile::tempdir().unwrap();
        let hang = dir.path().join("hang.sh");
        std::fs::write(&hang, "#!/bin/sh\nexec sleep 30\n").unwrap();
        std::fs::set_permissions(
            &hang,
            <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o755),
        )
        .unwrap();

        let mut installer = PacmanInstaller::new();
        installer.pacman = hang.to_string_lossy().to_string();
        installer.pkexec = hang.to_string_lossy().to_string();
        installer.use_polkit = false;
        installer.idle_timeout = std::time::Duration::from_millis(150);

        let package = dir.path().join("x.pkg.tar.zst");
        std::fs::write(&package, b"x").unwrap();

        let mut lines = Vec::new();
        let outcome = installer
            .install(&package, &mut |line| lines.push(line.to_string()))
            .unwrap();
        assert!(!outcome.success);
        assert_eq!(outcome.diagnosis, Some(Diagnosis::NotAuthorized));
        assert!(outcome
            .diagnosis_message
            .unwrap()
            .contains("tidak merespons"));
        assert!(lines.iter().any(|l| l.contains("tidak merespons")));
    }

    #[test]
    fn describe_shows_polkit_prefix() {
        let mut installer = PacmanInstaller::new();
        let args = vec!["-U".to_string(), "x.pkg.tar.zst".to_string()];
        assert_eq!(installer.describe(&args), "pkexec pacman -U x.pkg.tar.zst");
        installer.use_polkit = false;
        assert_eq!(installer.describe(&args), "pacman -U x.pkg.tar.zst");
    }
}
