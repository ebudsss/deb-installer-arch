//! Penulis `.MTREE` (format libarchive) yang dipakai pacman untuk verifikasi
//! integritas (`pacman -Qkk`).

use serde::{Deserialize, Serialize};
use std::io::Write;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MtreeKind {
    File,
    Dir,
    Link,
}

impl MtreeKind {
    fn as_str(self) -> &'static str {
        match self {
            MtreeKind::File => "file",
            MtreeKind::Dir => "dir",
            MtreeKind::Link => "link",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MtreeEntry {
    /// Path relatif tanpa awalan, mis. `usr/bin/app`.
    pub path: String,
    pub kind: MtreeKind,
    pub mode: u32,
    pub size: u64,
    /// Detik sejak epoch.
    pub time: i64,
    pub md5: Option<String>,
    pub sha256: Option<String>,
    /// Target untuk symlink.
    pub link: Option<String>,
}

/// Meng-escape path sesuai aturan mtree (spasi, backslash, karakter kontrol).
pub fn escape_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for byte in path.bytes() {
        match byte {
            b' ' => out.push_str("\\040"),
            b'\\' => out.push_str("\\\\"),
            b'\t' => out.push_str("\\011"),
            b'\n' => out.push_str("\\012"),
            b'#' => out.push_str("\\043"),
            0x00..=0x1f | 0x7f => out.push_str(&format!("\\{byte:03o}")),
            _ => out.push(byte as char),
        }
    }
    out
}

/// Membangun isi `.MTREE` (belum dikompresi).
pub fn render(entries: &[MtreeEntry]) -> String {
    let mut out = String::from("#mtree\n/set type=file uid=0 gid=0 mode=644\n");
    for entry in entries {
        out.push_str("./");
        out.push_str(&escape_path(entry.path.trim_start_matches("./")));
        out.push_str(&format!(" time={}.0", entry.time));
        if entry.kind != MtreeKind::Dir {
            out.push_str(&format!(" size={}", entry.size));
        }
        if let Some(md5) = &entry.md5 {
            out.push_str(&format!(" md5digest={md5}"));
        }
        if let Some(sha256) = &entry.sha256 {
            out.push_str(&format!(" sha256digest={sha256}"));
        }
        out.push_str(&format!(" mode={:o}", entry.mode));
        out.push_str(&format!(" type={}", entry.kind.as_str()));
        if let Some(link) = &entry.link {
            out.push_str(&format!(" link={}", escape_path(link)));
        }
        out.push('\n');
    }
    out
}

/// Menulis `.MTREE` terkompresi gzip ke sebuah writer.
pub fn write_gzip<W: Write>(entries: &[MtreeEntry], writer: W) -> std::io::Result<()> {
    let mut enc = flate2::write::GzEncoder::new(writer, flate2::Compression::best());
    enc.write_all(render(entries).as_bytes())?;
    enc.finish()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_special_characters() {
        assert_eq!(escape_path("usr/share/a b/c#d"), "usr/share/a\\040b/c\\043d");
        assert_eq!(escape_path("a\\b"), "a\\\\b");
    }

    #[test]
    fn renders_entries_with_hashes_and_links() {
        let entries = vec![
            MtreeEntry {
                path: "usr/bin/app".into(),
                kind: MtreeKind::File,
                mode: 0o755,
                size: 10,
                time: 1700000000,
                md5: Some("abc".into()),
                sha256: Some("def".into()),
                link: None,
            },
            MtreeEntry {
                path: "usr/lib/libapp.so".into(),
                kind: MtreeKind::Link,
                mode: 0o777,
                size: 0,
                time: 1700000000,
                md5: None,
                sha256: None,
                link: Some("libapp.so.1".into()),
            },
        ];
        let text = render(&entries);
        assert!(text.starts_with("#mtree\n/set type=file uid=0 gid=0 mode=644\n"));
        assert!(text.contains("./usr/bin/app time=1700000000.0 size=10 md5digest=abc sha256digest=def mode=755 type=file"));
        assert!(text.contains("./usr/lib/libapp.so time=1700000000.0 size=0 mode=777 type=link link=libapp.so.1"));
    }

    #[test]
    fn gzip_roundtrip() {
        use std::io::Read;
        let entries = vec![MtreeEntry {
            path: "usr/bin/x".into(),
            kind: MtreeKind::File,
            mode: 0o644,
            size: 0,
            time: 0,
            md5: None,
            sha256: None,
            link: None,
        }];
        let mut buf = Vec::new();
        write_gzip(&entries, &mut buf).unwrap();
        let mut dec = flate2::read::GzDecoder::new(&buf[..]);
        let mut text = String::new();
        dec.read_to_string(&mut text).unwrap();
        assert!(text.contains("./usr/bin/x"));
    }
}
