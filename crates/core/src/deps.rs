//! Pemetaan dependensi Debian → paket Arch.
//!
//! Urutan strategi:
//! 1. Tabel mapping lokal (`data/mapping.toml`).
//! 2. Pencarian pemilik file lewat `pacman -F` (nama `libfoo.so.1`).
//! 3. Pencocokan nama langsung (`libfoo3` → `foo`).
//! 4. Tidak ditemukan: serahkan keputusan ke pengguna.

use crate::control::{Dependency, Relationship};
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// Tabel mapping bawaan, ditanam di binari.
pub const DEFAULT_MAPPING: &str = include_str!("../data/mapping.toml");

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Confidence {
    High,
    Medium,
    Low,
}

impl Confidence {
    pub fn label(self) -> &'static str {
        match self {
            Confidence::High => "tinggi",
            Confidence::Medium => "sedang",
            Confidence::Low => "rendah",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DepStatus {
    /// Sudah terpasang di sistem.
    Installed,
    /// Tersedia di repo resmi Arch.
    Available,
    /// Hanya tersedia di AUR (perlu dibangun manual).
    AurOnly,
    /// Tidak ditemukan; pengguna harus memutuskan.
    Missing,
    /// Sengaja diabaikan (khusus Debian).
    Ignored,
}

impl DepStatus {
    pub fn label(self) -> &'static str {
        match self {
            DepStatus::Installed => "terpasang",
            DepStatus::Available => "tersedia",
            DepStatus::AurOnly => "AUR",
            DepStatus::Missing => "tidak ditemukan",
            DepStatus::Ignored => "diabaikan",
        }
    }

    pub fn is_blocking(self) -> bool {
        matches!(self, DepStatus::Missing | DepStatus::AurOnly)
    }
}

/// Dari mana sebuah pemetaan berasal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolutionSource {
    Mapping,
    FileSearch,
    NameMatch,
    Ignored,
    Unresolved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepResolution {
    /// Nama dependensi Debian sebagaimana tertulis.
    pub debian: String,
    /// Batasan versi Debian (hanya informasi).
    pub constraint: Option<String>,
    /// Alternatif Debian lain pada klausa yang sama.
    pub alternatives: Vec<String>,
    /// Nama paket Arch yang dipilih.
    pub arch_name: Option<String>,
    pub status: DepStatus,
    pub confidence: Confidence,
    pub source: ResolutionSource,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum MappingEntry {
    Simple(String),
    Detailed {
        arch: String,
        #[serde(default)]
        note: Option<String>,
        #[serde(default)]
        aur: bool,
    },
}

#[derive(Debug, Clone, Deserialize, Default)]
struct MappingFile {
    #[serde(default)]
    mapping: BTreeMap<String, MappingEntry>,
    #[serde(default)]
    ignored: IgnoredSection,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct IgnoredSection {
    #[serde(default)]
    names: Vec<String>,
}

/// Tabel mapping yang bisa digabung dengan milik pengguna.
#[derive(Debug, Clone, Default)]
pub struct MappingTable {
    entries: BTreeMap<String, MappingEntry>,
    ignored: Vec<String>,
}

impl MappingTable {
    pub fn builtin() -> Self {
        Self::from_toml(DEFAULT_MAPPING).expect("mapping.toml bawaan valid")
    }

    pub fn from_toml(text: &str) -> Result<Self> {
        let file: MappingFile =
            toml::from_str(text).map_err(|e| crate::Error::Other(format!("mapping.toml: {e}")))?;
        Ok(Self {
            entries: file.mapping,
            ignored: file.ignored.names,
        })
    }

    /// Menggabungkan mapping pengguna di atas mapping bawaan.
    pub fn merged_with(&self, other: &MappingTable) -> Self {
        let mut entries = self.entries.clone();
        entries.extend(other.entries.clone());
        let mut ignored = self.ignored.clone();
        for name in &other.ignored {
            if !ignored.contains(name) {
                ignored.push(name.clone());
            }
        }
        Self { entries, ignored }
    }

    pub fn lookup(&self, name: &str) -> Option<&MappingEntry> {
        self.entries.get(name)
    }

    pub fn is_ignored(&self, name: &str) -> bool {
        self.ignored.iter().any(|n| n == name)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Paket AUR sebagaimana dilaporkan RPC AUR.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AurPackage {
    pub name: String,
    pub version: String,
    pub description: String,
}

/// Asal sebuah saran alternatif.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PackageSource {
    Repo,
    Aur,
}

/// Paket Arch yang mungkin menggantikan `.deb` yang sedang dibuka.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alternative {
    pub name: String,
    pub source: PackageSource,
    pub version: Option<String>,
    pub description: Option<String>,
}

/// Antarmuka query sistem, dipisah agar bisa diuji tanpa pacman.
pub trait PackageQuery {
    fn is_installed(&mut self, name: &str) -> bool;
    fn in_repo(&mut self, name: &str) -> bool;
    /// Paket yang memiliki file `soname` (mis. `libfoo.so.1`).
    fn owner_of_file(&mut self, soname: &str) -> Option<String>;
    /// Nama paket repo yang persis sama.
    fn exact_repo_package(&mut self, name: &str) -> Option<String>;
    /// Info paket AUR; `None` bila tidak ada atau jaringan tidak dipakai.
    fn aur_info(&mut self, _name: &str) -> Option<AurPackage> {
        None
    }

    /// Mengisi cache AUR untuk banyak nama sekaligus. Implementasi nyata
    /// memakai satu permintaan HTTP agar tidak menunggu satu per satu.
    fn prefetch_aur(&mut self, _names: &[String]) {}
}

/// Implementasi nyata memakai binari `pacman` dan `curl` (untuk AUR).
#[derive(Debug, Default)]
pub struct SystemPacman {
    installed: HashMap<String, bool>,
    repo: HashMap<String, bool>,
    files: HashMap<String, Option<String>>,
    aur: HashMap<String, Option<AurPackage>>,
    /// Cache daftar paket repo; `None` berarti belum dimuat.
    repo_index: Option<Vec<String>>,
}

impl SystemPacman {
    pub fn new() -> Self {
        Self::default()
    }

    fn run(program: &str, args: &[&str]) -> Option<String> {
        let out = std::process::Command::new(program)
            .args(args)
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        Some(String::from_utf8_lossy(&out.stdout).to_string())
    }

    fn repo_index(&mut self) -> &Vec<String> {
        if self.repo_index.is_none() {
            let list = Self::run("pacman", &["-Ssq", "."])
                .map(|s| s.lines().map(|l| l.trim().to_string()).collect())
                .unwrap_or_default();
            self.repo_index = Some(list);
        }
        self.repo_index.as_ref().expect("baru diisi")
    }
}

impl SystemPacman {
    /// Satu permintaan RPC untuk banyak nama sekaligus (batas 100 per panggilan
    /// sesuai dokumentasi AUR).
    fn fetch_aur_batch(names: &[String]) -> Vec<AurPackage> {
        if names.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for chunk in names.chunks(100) {
            let mut url = String::from("https://aur.archlinux.org/rpc/v5/info?");
            for (index, name) in chunk.iter().enumerate() {
                if index > 0 {
                    url.push('&');
                }
                url.push_str("arg[]=");
                url.push_str(&urlencode(name));
            }
            let Ok(output) = std::process::Command::new("curl")
                .args(["-sSf", "--max-time", "10", &url])
                .output()
            else {
                continue;
            };
            if !output.status.success() {
                continue;
            }
            let text = String::from_utf8_lossy(&output.stdout);
            let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            let Some(results) = value.get("results").and_then(|r| r.as_array()) else {
                continue;
            };
            for item in results {
                let Some(name) = item.get("Name").and_then(|v| v.as_str()) else {
                    continue;
                };
                out.push(AurPackage {
                    name: name.to_string(),
                    version: item
                        .get("Version")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    description: item
                        .get("Description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                });
            }
        }
        out
    }

    fn fetch_aur(name: &str) -> Option<AurPackage> {
        let url = format!("https://aur.archlinux.org/rpc/v5/info?arg[]={name}");
        let out = std::process::Command::new("curl")
            .args(["-sSf", "--max-time", "6", &url])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&out.stdout);
        let value: serde_json::Value = serde_json::from_str(&text).ok()?;
        let first = value.get("results")?.as_array()?.first()?;
        Some(AurPackage {
            name: first.get("Name")?.as_str()?.to_string(),
            version: first
                .get("Version")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            description: first
                .get("Description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        })
    }
}

impl PackageQuery for SystemPacman {
    fn is_installed(&mut self, name: &str) -> bool {
        if let Some(v) = self.installed.get(name) {
            return *v;
        }
        let v = Self::run("pacman", &["-Qq", name])
            .map(|s| s.lines().any(|l| l.trim() == name))
            .unwrap_or(false);
        self.installed.insert(name.to_string(), v);
        v
    }

    fn in_repo(&mut self, name: &str) -> bool {
        if let Some(v) = self.repo.get(name) {
            return *v;
        }
        let v = self.repo_index().iter().any(|n| n == name);
        self.repo.insert(name.to_string(), v);
        v
    }

    fn owner_of_file(&mut self, soname: &str) -> Option<String> {
        if let Some(v) = self.files.get(soname) {
            return v.clone();
        }
        // `pacman -F` hanya bekerja bila database file sudah tersinkron
        // (`pacman -Fy`, butuh root). Bila belum, hasilnya kosong.
        let found = Self::run("pacman", &["-F", "--quiet", soname]).and_then(|out| {
            out.lines()
                .next()
                .and_then(|line| line.split('/').next())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        });
        self.files.insert(soname.to_string(), found.clone());
        found
    }

    fn exact_repo_package(&mut self, name: &str) -> Option<String> {
        if self.in_repo(name) {
            Some(name.to_string())
        } else {
            None
        }
    }

    fn aur_info(&mut self, name: &str) -> Option<AurPackage> {
        if let Some(v) = self.aur.get(name) {
            return v.clone();
        }
        let found = Self::fetch_aur(name);
        self.aur.insert(name.to_string(), found.clone());
        found
    }

    fn prefetch_aur(&mut self, names: &[String]) {
        let pending: Vec<String> = names
            .iter()
            .filter(|name| !self.aur.contains_key(*name))
            .cloned()
            .collect();
        if pending.is_empty() {
            return;
        }
        // Tandai belum ditemukan lebih dulu supaya kegagalan jaringan tidak
        // memicu permintaan berulang untuk nama yang sama.
        for name in &pending {
            self.aur.insert(name.clone(), None);
        }
        for found in Self::fetch_aur_batch(&pending) {
            self.aur.insert(found.name.clone(), Some(found));
        }
    }
}

/// Pemeta dependensi yang menggabungkan tabel dan query sistem.
pub struct DepMapper<Q: PackageQuery> {
    table: MappingTable,
    query: Q,
    /// Izinkan query jaringan (AUR).
    pub allow_network: bool,
}

impl<Q: PackageQuery> DepMapper<Q> {
    pub fn new(table: MappingTable, query: Q) -> Self {
        Self {
            table,
            query,
            allow_network: true,
        }
    }

    pub fn table(&self) -> &MappingTable {
        &self.table
    }

    pub fn query_mut(&mut self) -> &mut Q {
        &mut self.query
    }

    /// Memetakan semua klausa dependensi.
    ///
    /// Nama yang berpotensi perlu dicek di AUR dikumpulkan lebih dulu dan
    /// diambil dalam satu permintaan, bukan satu per dependensi.
    pub fn resolve_all(&mut self, relationships: &[Relationship]) -> Vec<DepResolution> {
        if self.allow_network {
            let mut candidates: Vec<String> = Vec::new();
            for rel in relationships {
                for alt in &rel.alternatives {
                    match self.table.lookup(&alt.name) {
                        Some(MappingEntry::Simple(arch)) => candidates.push(arch.clone()),
                        Some(MappingEntry::Detailed {
                            arch, aur: true, ..
                        }) => candidates.push(arch.clone()),
                        Some(MappingEntry::Detailed { .. }) | None => {
                            candidates.push(alt.name.clone())
                        }
                    }
                }
            }
            candidates.sort();
            candidates.dedup();
            self.query.prefetch_aur(&candidates);
        }

        relationships
            .iter()
            .map(|rel| self.resolve_relationship(rel))
            .collect()
    }

    pub fn resolve_relationship(&mut self, rel: &Relationship) -> DepResolution {
        let names: Vec<String> = rel.alternatives.iter().map(|d| d.name.clone()).collect();
        let constraint = rel.primary().op.map(|op| {
            format!(
                "{} {}",
                op.as_str(),
                rel.primary().version.clone().unwrap_or_default()
            )
        });

        let mut first_missing: Option<DepResolution> = None;
        for alt in &rel.alternatives {
            let res = self.resolve_dependency(alt, names.clone(), constraint.clone());
            if !matches!(res.status, DepStatus::Missing | DepStatus::AurOnly) {
                return res;
            }
            if first_missing.is_none() {
                first_missing = Some(res);
            }
        }
        first_missing.unwrap_or_else(|| DepResolution {
            debian: rel.primary().name.clone(),
            constraint,
            alternatives: names,
            arch_name: None,
            status: DepStatus::Missing,
            confidence: Confidence::Low,
            source: ResolutionSource::Unresolved,
            note: Some("klausa dependensi kosong".into()),
        })
    }

    fn resolve_dependency(
        &mut self,
        dep: &Dependency,
        alternatives: Vec<String>,
        constraint: Option<String>,
    ) -> DepResolution {
        let name = dep.name.clone();

        if self.table.is_ignored(&name) {
            return DepResolution {
                debian: name,
                constraint,
                alternatives,
                arch_name: None,
                status: DepStatus::Ignored,
                confidence: Confidence::High,
                source: ResolutionSource::Ignored,
                note: Some("khusus Debian; tidak dibutuhkan di Arch".into()),
            };
        }

        // 1. Tabel mapping.
        if let Some(entry) = self.table.lookup(&name).cloned() {
            let (arch, note, aur) = match entry {
                MappingEntry::Simple(arch) => (arch, None, false),
                MappingEntry::Detailed { arch, note, aur } => (arch, note, aur),
            };
            if self.query.is_installed(&arch) {
                return self.finish(
                    &name,
                    constraint,
                    alternatives,
                    Some(arch),
                    DepStatus::Installed,
                    Confidence::High,
                    ResolutionSource::Mapping,
                    note,
                );
            }
            if aur {
                return self.finish(
                    &name,
                    constraint,
                    alternatives,
                    Some(arch),
                    DepStatus::AurOnly,
                    Confidence::Medium,
                    ResolutionSource::Mapping,
                    note,
                );
            }
            if self.query.in_repo(&arch) {
                return self.finish(
                    &name,
                    constraint,
                    alternatives,
                    Some(arch),
                    DepStatus::Available,
                    Confidence::High,
                    ResolutionSource::Mapping,
                    note,
                );
            }
            if self.allow_network {
                if let Some(found) = self.query.aur_info(&arch) {
                    return self.finish(
                        &name,
                        constraint,
                        alternatives,
                        Some(found.name),
                        DepStatus::AurOnly,
                        Confidence::Medium,
                        ResolutionSource::Mapping,
                        note,
                    );
                }
            }
            return self.finish(
                &name,
                constraint,
                alternatives,
                Some(arch),
                DepStatus::Missing,
                Confidence::Low,
                ResolutionSource::Mapping,
                Some("dipetakan ke paket yang tidak tersedia di sistem ini".into()),
            );
        }

        // 2. Pencarian pemilik file via soname.
        if let Some(soname) = soname_from_package(&name) {
            if let Some(owner) = self.query.owner_of_file(&soname) {
                let status = if self.query.is_installed(&owner) {
                    DepStatus::Installed
                } else {
                    DepStatus::Available
                };
                return self.finish(
                    &name,
                    constraint,
                    alternatives,
                    Some(owner),
                    status,
                    Confidence::Medium,
                    ResolutionSource::FileSearch,
                    Some(format!("pemilik {soname} menurut pacman -F")),
                );
            }
        }

        // 3. Pencocokan nama langsung.
        for candidate in name_candidates(&name) {
            if self.query.is_installed(&candidate) {
                return self.finish(
                    &name,
                    constraint,
                    alternatives,
                    Some(candidate),
                    DepStatus::Installed,
                    Confidence::Medium,
                    ResolutionSource::NameMatch,
                    None,
                );
            }
            if let Some(found) = self.query.exact_repo_package(&candidate) {
                return self.finish(
                    &name,
                    constraint,
                    alternatives,
                    Some(found),
                    DepStatus::Available,
                    Confidence::Medium,
                    ResolutionSource::NameMatch,
                    None,
                );
            }
        }

        // 4. Tidak ditemukan.
        let mut note =
            String::from("tidak ada padanan otomatis; pilih paket Arch secara manual atau abaikan");
        if self.allow_network {
            if let Some(aur) = self.query.aur_info(&name) {
                note = format!("tersedia di AUR sebagai `{}`", aur.name);
                return self.finish(
                    &name,
                    constraint,
                    alternatives,
                    Some(aur.name),
                    DepStatus::AurOnly,
                    Confidence::Low,
                    ResolutionSource::NameMatch,
                    Some(note),
                );
            }
        }
        self.finish(
            &name,
            constraint,
            alternatives,
            None,
            DepStatus::Missing,
            Confidence::Low,
            ResolutionSource::Unresolved,
            Some(note),
        )
    }

    /// Mencari paket Arch yang mungkin sudah tersedia untuk `.deb` ini,
    /// sehingga konversi tidak perlu dilakukan.
    pub fn suggest_alternatives(&mut self, deb_name: &str) -> Vec<Alternative> {
        let mut out: Vec<Alternative> = Vec::new();
        let mut seen = std::collections::BTreeSet::new();

        for candidate in name_candidates(deb_name) {
            if !seen.insert(candidate.clone()) {
                continue;
            }
            if self.query.in_repo(&candidate) {
                out.push(Alternative {
                    name: candidate,
                    source: PackageSource::Repo,
                    version: None,
                    description: None,
                });
            }
        }

        if self.allow_network && out.is_empty() {
            if let Some(aur) = self.query.aur_info(deb_name) {
                out.push(Alternative {
                    name: aur.name,
                    source: PackageSource::Aur,
                    version: Some(aur.version),
                    description: Some(aur.description),
                });
            }
        }
        out
    }

    #[allow(clippy::too_many_arguments)]
    fn finish(
        &self,
        debian: &str,
        constraint: Option<String>,
        alternatives: Vec<String>,
        arch_name: Option<String>,
        status: DepStatus,
        confidence: Confidence,
        source: ResolutionSource,
        note: Option<String>,
    ) -> DepResolution {
        DepResolution {
            debian: debian.to_string(),
            constraint,
            alternatives,
            arch_name,
            status,
            confidence,
            source,
            note,
        }
    }
}

/// Persen-encode parameter query AUR. Nama paket Debian boleh memuat `+`,
/// `-`, dan `:` yang harus di-escape.
fn urlencode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Menebak soname dari nama paket library Debian:
/// `libfoo3` → `libfoo.so.3`, `libbar-1.0-0` → `libbar-1.0.so.0`.
pub fn soname_from_package(name: &str) -> Option<String> {
    let rest = name.strip_prefix("lib")?;
    let digits: String = rest
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    if digits.is_empty() {
        return None;
    }
    let stem = rest.trim_end_matches(|c: char| c.is_ascii_digit());
    if stem.is_empty() {
        return None;
    }
    Some(format!("lib{stem}.so.{digits}"))
}

/// Kandidat nama paket Arch untuk sebuah nama Debian.
pub fn name_candidates(name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut base = name.to_string();

    // `libfoo3` → `foo`, `libfoo3` (tanpa lib).
    if let Some(stem) = base.strip_prefix("lib") {
        let trimmed = stem.trim_end_matches(|c: char| c.is_ascii_digit());
        if !trimmed.is_empty() && trimmed != stem {
            out.push(trimmed.to_string());
            out.push(format!("lib{trimmed}"));
        }
    }
    // `foo-dev` → `foo`, `foo-doc` → `foo`.
    for suffix in ["-dev", "-doc", "-dbg", "-dbgsym", "-common"] {
        if let Some(stem) = base.strip_suffix(suffix) {
            out.push(stem.to_string());
        }
    }
    // Buang qualifier `:any`.
    if let Some((head, _)) = base.split_once(':') {
        base = head.to_string();
    }
    out.push(base);
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[derive(Default, Clone)]
    struct FakeQuery {
        installed: HashSet<String>,
        repo: HashSet<String>,
        files: HashMap<String, String>,
        aur: HashMap<String, AurPackage>,
    }

    impl PackageQuery for FakeQuery {
        fn is_installed(&mut self, name: &str) -> bool {
            self.installed.contains(name)
        }
        fn in_repo(&mut self, name: &str) -> bool {
            self.repo.contains(name)
        }
        fn owner_of_file(&mut self, soname: &str) -> Option<String> {
            self.files.get(soname).cloned()
        }
        fn exact_repo_package(&mut self, name: &str) -> Option<String> {
            self.repo.contains(name).then(|| name.to_string())
        }
        fn aur_info(&mut self, name: &str) -> Option<AurPackage> {
            self.aur.get(name).cloned()
        }
    }

    fn mapper(q: FakeQuery) -> DepMapper<FakeQuery> {
        DepMapper::new(MappingTable::builtin(), q)
    }

    fn rel(s: &str) -> Relationship {
        Relationship::parse_list(s).remove(0)
    }

    #[test]
    fn builtin_mapping_loads_and_is_large() {
        let table = MappingTable::builtin();
        assert!(
            table.len() > 100,
            "mapping bawaan terlalu kecil: {}",
            table.len()
        );
        assert!(table.is_ignored("dpkg"));
    }

    #[test]
    fn resolves_mapping_to_installed_package() {
        let mut q = FakeQuery::default();
        q.installed.insert("gtk3".into());
        let mut m = mapper(q);
        let res = m.resolve_relationship(&rel("libgtk-3-0 (>= 3.24)"));
        assert_eq!(res.arch_name.as_deref(), Some("gtk3"));
        assert_eq!(res.status, DepStatus::Installed);
        assert_eq!(res.confidence, Confidence::High);
        assert_eq!(res.source, ResolutionSource::Mapping);
        assert_eq!(res.constraint.as_deref(), Some(">= 3.24"));
    }

    #[test]
    fn resolves_mapping_to_repo_package() {
        let mut q = FakeQuery::default();
        q.repo.insert("glibc".into());
        let mut m = mapper(q);
        let res = m.resolve_relationship(&rel("libc6"));
        assert_eq!(res.status, DepStatus::Available);
        assert_eq!(res.arch_name.as_deref(), Some("glibc"));
    }

    #[test]
    fn ignored_dependency_is_marked_ignored() {
        let mut m = mapper(FakeQuery::default());
        let res = m.resolve_relationship(&rel("dpkg (>= 1.14)"));
        assert_eq!(res.status, DepStatus::Ignored);
        assert!(!res.status.is_blocking());
    }

    #[test]
    fn alternative_picks_first_resolvable() {
        let mut q = FakeQuery::default();
        q.installed.insert("gtk4".into());
        let mut m = mapper(q);
        let res = m.resolve_relationship(&rel("libgtk-3-0 | libgtk-4-1"));
        assert_eq!(res.arch_name.as_deref(), Some("gtk4"));
        assert_eq!(res.status, DepStatus::Installed);
        assert_eq!(res.alternatives.len(), 2);
    }

    #[test]
    fn falls_back_to_file_search_for_unknown_library() {
        let mut q = FakeQuery::default();
        q.files.insert("libweird.so.2".into(), "weird-libs".into());
        q.repo.insert("weird-libs".into());
        let mut m = mapper(q);
        let res = m.resolve_relationship(&rel("libweird2"));
        assert_eq!(res.arch_name.as_deref(), Some("weird-libs"));
        assert_eq!(res.source, ResolutionSource::FileSearch);
        assert_eq!(res.confidence, Confidence::Medium);
        assert_eq!(res.status, DepStatus::Available);
    }

    #[test]
    fn falls_back_to_name_match() {
        let mut q = FakeQuery::default();
        q.repo.insert("foo".into());
        let mut m = mapper(q);
        let res = m.resolve_relationship(&rel("foo-dev"));
        assert_eq!(res.arch_name.as_deref(), Some("foo"));
        assert_eq!(res.source, ResolutionSource::NameMatch);
    }

    #[test]
    fn aur_only_is_blocking_and_noted() {
        let mut q = FakeQuery::default();
        q.aur.insert(
            "openssl-1.1".into(),
            AurPackage {
                name: "openssl-1.1".into(),
                version: "1.1.1w-1".into(),
                description: "OpenSSL 1.1".into(),
            },
        );
        let mut m = mapper(q);
        let res = m.resolve_relationship(&rel("libssl1.1"));
        assert_eq!(res.status, DepStatus::AurOnly);
        assert_eq!(res.arch_name.as_deref(), Some("openssl-1.1"));
        assert!(res.status.is_blocking());
    }

    #[test]
    fn unresolved_dependency_is_missing() {
        let mut m = mapper(FakeQuery::default());
        let res = m.resolve_relationship(&rel("some-obscure-pkg"));
        assert_eq!(res.status, DepStatus::Missing);
        assert_eq!(res.source, ResolutionSource::Unresolved);
        assert!(res.arch_name.is_none());
        assert!(res.note.is_some());
    }

    #[test]
    fn soname_guessing() {
        assert_eq!(
            soname_from_package("libnss3").as_deref(),
            Some("libnss.so.3")
        );
        assert_eq!(
            soname_from_package("libavcodec60").as_deref(),
            Some("libavcodec.so.60")
        );
        assert_eq!(
            soname_from_package("libgtk-3-0").as_deref(),
            Some("libgtk-3-.so.0")
        );
        assert_eq!(soname_from_package("zlib1g"), None);
        assert_eq!(soname_from_package("libc6"), Some("libc.so.6".into()));
    }

    #[test]
    fn user_mapping_overrides_builtin() {
        let user = MappingTable::from_toml(
            r#"
            [mapping]
            "libgtk-3-0" = "gtk3-custom"
            "my-lib" = { arch = "my-lib-aur", aur = true, note = "AUR" }
            [ignored]
            names = ["custom-ignored"]
            "#,
        )
        .unwrap();
        let merged = MappingTable::builtin().merged_with(&user);
        assert!(merged.is_ignored("custom-ignored"));
        assert!(merged.is_ignored("dpkg"));
        match merged.lookup("my-lib") {
            Some(MappingEntry::Detailed { arch, aur, .. }) => {
                assert_eq!(arch, "my-lib-aur");
                assert!(*aur);
            }
            other => panic!("entri tak terduga: {other:?}"),
        }

        let mut q = FakeQuery::default();
        q.repo.insert("gtk3-custom".into());
        let mut m = DepMapper::new(merged, q);
        let res = m.resolve_relationship(&rel("libgtk-3-0"));
        assert_eq!(res.arch_name.as_deref(), Some("gtk3-custom"));
    }

    #[test]
    fn system_pacman_parses_installed_packages() {
        // Query nyata terhadap sistem: `bash` hampir pasti terpasang.
        let mut q = SystemPacman::new();
        assert!(q.is_installed("bash"));
        assert!(!q.is_installed("paket-yang-tidak-mungkin-ada-xyz"));
    }
}
