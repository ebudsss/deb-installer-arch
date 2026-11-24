use std::fmt;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("kesalahan I/O: {0}")]
    Io(#[from] std::io::Error),

    #[error("bukan arsip .deb: magic `!<arch>` tidak ditemukan")]
    NotAr,

    #[error("header ar rusak pada offset {0}")]
    BadArHeader(u64),

    #[error("anggota arsip `{0}` tidak ditemukan di dalam .deb")]
    MissingMember(String),

    #[error("format arsip tidak didukung: {0}")]
    UnsupportedArchive(String),

    #[error("file control tidak valid: {0}")]
    BadControl(String),

    #[error("field wajib `{0}` tidak ada di file control")]
    MissingField(&'static str),

    #[error("path berbahaya di dalam paket: {0}")]
    UnsafePath(String),

    #[error("paket tidak bisa dikonversi: {0}")]
    Build(String),

    #[error("instalasi gagal: {0}")]
    Install(String),

    #[error("kesalahan JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("{0}")]
    Other(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Ringkasan kegagalan perintah eksternal (pacman/pkexec).
#[derive(Debug, Clone)]
pub struct CommandFailure {
    pub program: String,
    pub args: Vec<String>,
    pub status: Option<i32>,
    pub stderr: String,
}

impl fmt::Display for CommandFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "`{} {}` keluar dengan status {:?}: {}",
            self.program,
            self.args.join(" "),
            self.status,
            self.stderr.trim()
        )
    }
}

/// Membungkus kegagalan perintah agar pesan ke pengguna tetap jelas.
pub fn command_error(program: &str, args: &[String], status: Option<i32>, stderr: String) -> Error {
    Error::Other(
        CommandFailure {
            program: program.to_string(),
            args: args.to_vec(),
            status,
            stderr,
        }
        .to_string(),
    )
}
