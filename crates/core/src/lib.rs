//! Library core untuk DebInstaller.
//!
//! Alur: baca `.deb` → petakan dependensi → analisis script → bangun
//! `.pkg.tar.zst` → pasang lewat `pacman -U` (polkit).
//!
//! Semua operasi berat berjalan sebagai user biasa; hanya `pacman -U` yang
//! meminta hak akses lewat `pkexec`.

pub mod ar;
pub mod archive;
pub mod control;
pub mod deb;
pub mod deps;
pub mod error;
pub mod install;
pub mod launch;
pub mod mtree;
pub mod normalize;
pub mod pkgbuild;
pub mod scripts;
pub mod security;
pub mod state;

pub use archive::{ArchiveLayout, ArchivePackage};
pub use control::Control;
pub use deb::DebPackage;
pub use deps::{
    Alternative, AurPackage, Confidence, DepMapper, DepResolution, DepStatus, MappingTable,
    PackageQuery, PackageSource, SystemPacman,
};
pub use error::{Error, Result};
pub use install::{Diagnosis, InstallOutcome, PacmanInstaller};
pub use launch::{
    discover as discover_launcher, launch as launch_application, LaunchKind, LaunchTarget,
};
pub use pkgbuild::{ArchVersion, BuildOptions, BuildResult, BuildStage, PackageBuilder};
pub use scripts::{Finding, Risk, ScriptReport};
