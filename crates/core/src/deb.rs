//! Pembacaan paket `.deb`: arsip `ar` berisi `debian-binary`, `control.tar.*`,
//! dan `data.tar.*`.

use crate::ar::{self, ArEntryRef};
use crate::control::Control;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

/// Kompresi yang dipakai anggota tar di dalam `.deb`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Compression {
    None,
    Gzip,
    Xz,
    Zstd,
}

impl Compression {
    /// Deteksi dari magic byte.
    pub fn detect(head: &[u8]) -> Option<Self> {
        if head.starts_with(&[0x1f, 0x8b]) {
            Some(Compression::Gzip)
        } else if head.starts_with(&[0xfd, b'7', b'z', b'X', b'Z', 0x00]) {
            Some(Compression::Xz)
        } else if head.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]) {
            Some(Compression::Zstd)
        } else if head.len() >= 262 && &head[257..262] == b"ustar" {
            Some(Compression::None)
        } else {
            None
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        if name.ends_with(".gz") {
            Some(Compression::Gzip)
        } else if name.ends_with(".xz") || name.ends_with(".lzma") {
            Some(Compression::Xz)
        } else if name.ends_with(".zst") || name.ends_with(".zstd") {
            Some(Compression::Zstd)
        } else if name.ends_with(".tar") {
            Some(Compression::None)
        } else {
            None
        }
    }

    fn wrap<'a>(self, reader: Box<dyn Read + 'a>) -> Result<Box<dyn Read + 'a>> {
        Ok(match self {
            Compression::None => reader,
            Compression::Gzip => Box::new(flate2::read::GzDecoder::new(reader)),
            Compression::Xz => Box::new(xz2::read::XzDecoder::new(reader)),
            Compression::Zstd => Box::new(
                zstd::stream::read::Decoder::new(reader)
                    .map_err(|e| Error::UnsupportedArchive(format!("zstd: {e}")))?,
            ),
        })
    }
}

/// Satu entri di dalam `data.tar`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    /// Path relatif tanpa awalan `./` atau `/`.
    pub path: String,
    pub kind: FileKind,
    pub size: u64,
    pub mode: u32,
    /// Target untuk symlink/hardlink.
    pub link_target: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileKind {
    File,
    Directory,
    Symlink,
    Hardlink,
}

/// Paket `.deb` yang sudah dianalisis.
#[derive(Debug, Clone)]
pub struct DebPackage {
    pub path: PathBuf,
    pub file_size: u64,
    pub sha256: String,
    pub control: Control,
    /// Isi `control.tar.*` apa adanya (control, postinst, md5sums, ...).
    pub control_files: BTreeMap<String, Vec<u8>>,
    control_entry: ArEntryRef,
    data_entry: ArEntryRef,
    data_compression: Compression,
}

impl DebPackage {
    /// Membuka dan memvalidasi struktur `.deb`. Hanya `control.tar.*` yang
    /// dimuat ke memori; `data.tar.*` dibaca streaming saat dibutuhkan.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let meta = std::fs::metadata(&path)?;
        if !meta.is_file() {
            return Err(Error::Other(format!("{} bukan file biasa", path.display())));
        }

        let mut file = BufReader::new(std::fs::File::open(&path)?);
        let entries = ar::index_archive(&mut file)?;

        let control_entry = entries
            .iter()
            .find(|e| e.name.starts_with("control.tar"))
            .cloned()
            .ok_or_else(|| Error::MissingMember("control.tar.*".into()))?;
        let data_entry = entries
            .iter()
            .find(|e| e.name.starts_with("data.tar"))
            .cloned()
            .ok_or_else(|| Error::MissingMember("data.tar.*".into()))?;

        let control_files = Self::read_control_tar(&path, &control_entry)?;
        let control_raw = control_files
            .get("control")
            .ok_or_else(|| Error::MissingMember("control.tar/./control".into()))?;
        let control = Control::parse(&String::from_utf8_lossy(control_raw))?;

        let data_compression = Self::detect_member_compression(&path, &data_entry)
            .or_else(|| Compression::from_name(&data_entry.name))
            .ok_or_else(|| {
                Error::UnsupportedArchive(format!("kompresi `{}` tidak dikenal", data_entry.name))
            })?;

        let sha256 = hash_file(&path)?;

        Ok(Self {
            path,
            file_size: meta.len(),
            sha256,
            control,
            control_files,
            control_entry,
            data_entry,
            data_compression,
        })
    }

    fn detect_member_compression(path: &Path, entry: &ArEntryRef) -> Option<Compression> {
        let mut reader = entry.reader(path).ok()?;
        let mut head = [0u8; 300];
        let n = reader.read(&mut head).ok()?;
        Compression::detect(&head[..n])
    }

    fn read_control_tar(path: &Path, entry: &ArEntryRef) -> Result<BTreeMap<String, Vec<u8>>> {
        let raw = entry.read(path)?;
        let compression = Compression::detect(&raw)
            .or_else(|| Compression::from_name(&entry.name))
            .ok_or_else(|| {
                Error::UnsupportedArchive(format!("kompresi `{}` tidak dikenal", entry.name))
            })?;
        let reader = compression.wrap(Box::new(std::io::Cursor::new(raw)))?;
        let mut archive = tar::Archive::new(reader);
        let mut files = BTreeMap::new();

        for entry in archive.entries()? {
            let mut entry = entry?;
            let kind = entry.header().entry_type();
            if !(kind.is_file() || kind.is_gnu_longname() || kind.is_contiguous()) {
                continue;
            }
            let path = entry.path()?.to_string_lossy().to_string();
            let name = path
                .trim_start_matches("./")
                .trim_start_matches('/')
                .to_string();
            if name.is_empty() {
                continue;
            }
            let mut buf = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut buf)?;
            files.insert(name, buf);
        }

        Ok(files)
    }

    /// Reader streaming untuk `data.tar` yang sudah didekompresi.
    pub fn data_reader(&self) -> Result<Box<dyn Read + '_>> {
        let raw = self.data_entry.reader(&self.path)?;
        self.data_compression.wrap(Box::new(raw))
    }

    /// Nama anggota data tar, mis. `data.tar.zst`.
    pub fn data_member_name(&self) -> &str {
        &self.data_entry.name
    }

    pub fn control_member_name(&self) -> &str {
        &self.control_entry.name
    }

    pub fn data_compression(&self) -> Compression {
        self.data_compression
    }

    /// Daftar isi `data.tar` tanpa mengekstrak ke disk.
    pub fn data_entries(&self) -> Result<Vec<FileEntry>> {
        let reader = self.data_reader()?;
        let mut archive = tar::Archive::new(reader);
        let mut out = Vec::new();

        for entry in archive.entries()? {
            let entry = entry?;
            let header = entry.header();
            let entry_type = header.entry_type();
            let path = entry
                .path()?
                .to_string_lossy()
                .trim_start_matches("./")
                .trim_start_matches('/')
                .to_string();
            if path.is_empty() {
                continue;
            }
            let mode = header.mode().unwrap_or(0o644);
            let size = header.size().unwrap_or(0);
            let link_target = entry
                .link_name()
                .ok()
                .flatten()
                .map(|p| p.to_string_lossy().to_string());

            let kind = if entry_type.is_dir() {
                FileKind::Directory
            } else if entry_type.is_symlink() {
                FileKind::Symlink
            } else if entry_type.is_hard_link() {
                FileKind::Hardlink
            } else {
                FileKind::File
            };

            out.push(FileEntry {
                path,
                kind,
                size,
                mode,
                link_target,
            });
        }

        Ok(out)
    }

    /// Script maintainer Debian yang tersedia, sebagai teks.
    pub fn maintainer_scripts(&self) -> BTreeMap<String, String> {
        const NAMES: [&str; 5] = ["preinst", "postinst", "prerm", "postrm", "config"];
        let mut out = BTreeMap::new();
        for name in NAMES {
            if let Some(bytes) = self.control_files.get(name) {
                out.insert(name.to_string(), String::from_utf8_lossy(bytes).to_string());
            }
        }
        out
    }

    /// Total ukuran file yang akan terpasang.
    pub fn installed_size(&self) -> Result<u64> {
        Ok(self
            .data_entries()?
            .iter()
            .filter(|e| e.kind == FileKind::File)
            .map(|e| e.size)
            .sum())
    }
}

pub fn hash_file(path: &Path) -> Result<String> {
    let mut file = BufReader::new(std::fs::File::open(path)?);
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn detects_compression_magics() {
        assert_eq!(
            Compression::detect(&[0x1f, 0x8b, 0x08]),
            Some(Compression::Gzip)
        );
        assert_eq!(
            Compression::detect(&[0xfd, b'7', b'z', b'X', b'Z', 0x00]),
            Some(Compression::Xz)
        );
        assert_eq!(
            Compression::detect(&[0x28, 0xb5, 0x2f, 0xfd]),
            Some(Compression::Zstd)
        );
        assert_eq!(Compression::detect(b"garbage"), None);
    }

    #[test]
    fn names_map_to_compression() {
        assert_eq!(
            Compression::from_name("data.tar.zst"),
            Some(Compression::Zstd)
        );
        assert_eq!(Compression::from_name("data.tar.xz"), Some(Compression::Xz));
        assert_eq!(
            Compression::from_name("control.tar.gz"),
            Some(Compression::Gzip)
        );
        assert_eq!(Compression::from_name("weird.bin"), None);
    }

    /// Membangun .deb minimal di disk untuk menguji jalur baca.
    fn write_test_deb(path: &Path, control_tar_gz: &[u8], data_tar_xz: &[u8]) {
        let mut out = std::fs::File::create(path).unwrap();
        out.write_all(b"!<arch>\n").unwrap();
        let mut member = |name: &str, data: &[u8]| {
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
        };
        member("debian-binary", b"2.0\n");
        member("control.tar.gz", control_tar_gz);
        member("data.tar.xz", data_tar_xz);
    }

    fn gz_tar(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut tar_buf = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut tar_buf);
            for (name, content) in files {
                let mut header = tar::Header::new_gnu();
                header.set_size(content.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                builder.append_data(&mut header, name, *content).unwrap();
            }
            builder.finish().unwrap();
        }
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        enc.write_all(&tar_buf).unwrap();
        enc.finish().unwrap()
    }

    #[test]
    fn opens_deb_and_lists_contents() {
        let dir = tempfile::tempdir().unwrap();
        let control = b"Package: hello\nVersion: 1.0-1\nArchitecture: amd64\nDepends: libc6 (>= 2.34)\nDescription: Demo package\n Long text.\n";
        let control_tar = gz_tar(&[("./control", control), ("./postinst", b"#!/bin/sh\n")]);
        let data_tar = gz_tar(&[("./usr/bin/hello", b"#!/bin/sh\necho hi\n")]);
        let path = dir.path().join("hello.deb");
        write_test_deb(&path, &control_tar, &data_tar);

        let deb = DebPackage::open(&path).unwrap();
        assert_eq!(deb.control.package().unwrap(), "hello");
        assert_eq!(deb.control.version().unwrap(), "1.0-1");
        assert_eq!(deb.data_compression(), Compression::Gzip);
        assert_eq!(deb.control_member_name(), "control.tar.gz");
        assert!(deb.sha256.len() == 64);

        let entries = deb.data_entries().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "usr/bin/hello");
        assert_eq!(entries[0].kind, FileKind::File);

        let scripts = deb.maintainer_scripts();
        assert!(scripts.contains_key("postinst"));
        assert_eq!(deb.installed_size().unwrap(), 18);
    }

    #[test]
    fn rejects_archive_without_data_member() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("broken.deb");
        let mut out = std::fs::File::create(&path).unwrap();
        out.write_all(b"!<arch>\n").unwrap();
        drop(out);
        assert!(matches!(
            DebPackage::open(&path),
            Err(Error::MissingMember(_))
        ));
    }
}
