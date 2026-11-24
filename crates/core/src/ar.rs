//! Pembaca arsip `ar` (format yang dipakai `.deb`).
//!
//! Format arsip `ar` klasik, header 60 byte:
//! ```text
//! offset  len  isi
//!      0   16   nama (diakhiri '/', GNU: "//" = tabel nama panjang)
//!     16   12   mtime desimal
//!     28    6   uid
//!     34    6   gid
//!     40    8   mode oktal
//!     48   10   ukuran desimal
//!     58    2   "`\n"
//! ```

use crate::error::{Error, Result};
use std::io::{Read, Seek, SeekFrom};

const HEADER_LEN: u64 = 60;
const MAGIC: &[u8; 8] = b"!<arch>\n";

/// Anggota arsip dengan isi yang sudah dimuat ke memori.
#[derive(Debug, Clone)]
pub struct ArMember {
    /// Nama tanpa akhiran `/` (GNU juga tanpa akhiran `/\n`).
    pub name: String,
    pub data: Vec<u8>,
}

/// Lokasi satu anggota di dalam file arsip, tanpa memuat isinya.
#[derive(Debug, Clone)]
pub struct ArEntryRef {
    pub name: String,
    pub offset: u64,
    pub size: u64,
}

impl ArEntryRef {
    /// Memuat isi anggota dari file arsip.
    pub fn read(&self, path: &std::path::Path) -> Result<Vec<u8>> {
        let mut file = std::fs::File::open(path)?;
        file.seek(SeekFrom::Start(self.offset))?;
        let mut buf = vec![0u8; self.size as usize];
        file.read_exact(&mut buf)?;
        Ok(buf)
    }

    /// Reader terbatas pada isi anggota, untuk dekompresi streaming.
    pub fn reader(&self, path: &std::path::Path) -> Result<impl Read> {
        let mut file = std::fs::File::open(path)?;
        file.seek(SeekFrom::Start(self.offset))?;
        Ok(file.take(self.size))
    }
}

/// Mengindeks anggota arsip tanpa memuat payload ke memori.
pub fn index_archive<R: Read + Seek>(reader: &mut R) -> Result<Vec<ArEntryRef>> {
    let mut magic = [0u8; 8];
    reader.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(Error::NotAr);
    }

    let mut entries = Vec::new();
    let mut long_names: Vec<u8> = Vec::new();

    loop {
        let header_offset = reader.stream_position()?;
        let mut header = [0u8; HEADER_LEN as usize];
        match reader.read_exact(&mut header) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.into()),
        }
        if &header[58..60] != b"`\n" {
            return Err(Error::BadArHeader(header_offset));
        }
        let raw_name = std::str::from_utf8(&header[0..16])
            .map_err(|_| Error::BadArHeader(header_offset))?
            .trim();
        let size: u64 = std::str::from_utf8(&header[48..58])
            .map_err(|_| Error::BadArHeader(header_offset))?
            .trim()
            .parse()
            .map_err(|_| Error::BadArHeader(header_offset))?;
        if size > 1 << 32 {
            return Err(Error::UnsupportedArchive(format!(
                "anggota `{raw_name}` terlalu besar ({size} byte)"
            )));
        }

        let offset = reader.stream_position()?;
        reader.seek(SeekFrom::Current(size as i64 + (size % 2) as i64))?;

        match raw_name {
            "//" => {
                let mut buf = vec![0u8; size as usize];
                let back = reader.stream_position()?;
                reader.seek(SeekFrom::Start(offset))?;
                reader.read_exact(&mut buf)?;
                reader.seek(SeekFrom::Start(back))?;
                long_names = buf;
                continue;
            }
            "/" | "__.SYMDEF" | "__.SYMDEF SORTED" => continue,
            _ => {}
        }

        // Nama GNU "#1/<len>" menyimpan nama di awal payload, bukan payload file.
        if let Some(rest) = raw_name.strip_prefix("#1/") {
            let name_len: u64 = rest.parse().map_err(|_| Error::BadArHeader(0))?;
            if name_len > size {
                return Err(Error::BadArHeader(offset));
            }
            let mut name_buf = vec![0u8; name_len as usize];
            reader.seek(SeekFrom::Start(offset))?;
            reader.read_exact(&mut name_buf)?;
            let name = String::from_utf8_lossy(&name_buf)
                .trim_end_matches('\0')
                .trim()
                .to_string();
            entries.push(ArEntryRef {
                name,
                offset: offset + name_len,
                size: size - name_len,
            });
            continue;
        }

        let name = resolve_name(raw_name, &long_names, &[])?;
        entries.push(ArEntryRef { name, offset, size });
    }

    Ok(entries)
}

/// Membaca seluruh anggota arsip `ar` dari sebuah reader yang mendukung seek.
///
/// Anggota padding (byte `\n`) dan simbol tabel GNU (`/`) dilewati.
pub fn read_archive<R: Read + Seek>(reader: &mut R) -> Result<Vec<ArMember>> {
    let mut magic = [0u8; 8];
    reader.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(Error::NotAr);
    }

    let mut members: Vec<ArMember> = Vec::new();
    let mut long_names: Vec<u8> = Vec::new();

    loop {
        let header_offset = reader.stream_position()?;
        let mut header = [0u8; HEADER_LEN as usize];
        match reader.read_exact(&mut header) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.into()),
        }

        if &header[58..60] != b"`\n" {
            return Err(Error::BadArHeader(header_offset));
        }

        let raw_name = std::str::from_utf8(&header[0..16])
            .map_err(|_| Error::BadArHeader(header_offset))?
            .trim();
        let size_str = std::str::from_utf8(&header[48..58])
            .map_err(|_| Error::BadArHeader(header_offset))?
            .trim();
        let size: u64 = size_str
            .parse()
            .map_err(|_| Error::BadArHeader(header_offset))?;

        if size > 1 << 32 {
            return Err(Error::UnsupportedArchive(format!(
                "anggota `{raw_name}` terlalu besar ({size} byte)"
            )));
        }

        let mut data = vec![0u8; size as usize];
        reader.read_exact(&mut data)?;

        // Anggota selalu diakhiri padding ganjil.
        if size % 2 == 1 {
            let mut pad = [0u8; 1];
            if reader.read_exact(&mut pad).is_err() {
                // Padding hilang di akhir file: masih dapat diterima.
            }
        }

        match raw_name {
            // Tabel nama panjang GNU: anggota berikutnya merujuk offset ke sini.
            "//" => {
                long_names = data;
                continue;
            }
            // Tabel simbol: tidak dibutuhkan.
            "/" => continue,
            // Metadata format BSD, bukan bagian .deb.
            "__.SYMDEF" | "__.SYMDEF SORTED" => continue,
            _ => {}
        }

        let name = resolve_name(raw_name, &long_names, &data)?;
        members.push(ArMember { name, data });
    }

    Ok(members)
}

fn resolve_name(raw: &str, long_names: &[u8], data: &[u8]) -> Result<String> {
    // GNU: "#1/<len>" → nama disimpan di awal payload.
    if let Some(rest) = raw.strip_prefix("#1/") {
        let len: usize = rest
            .parse()
            .map_err(|_| Error::BadArHeader(0))?;
        let name = data.get(..len).ok_or(Error::BadArHeader(0))?;
        let name = String::from_utf8_lossy(name)
            .trim_end_matches('\0')
            .trim_end()
            .to_string();
        return Ok(name);
    }

    // GNU: "/<offset>" ke tabel nama panjang.
    if let Some(offset) = raw.strip_prefix('/').filter(|s| !s.is_empty()) {
        let offset: usize = offset.parse().map_err(|_| Error::BadArHeader(0))?;
        let slice = long_names.get(offset..).ok_or(Error::BadArHeader(0))?;
        let end = slice
            .iter()
            .position(|&b| b == b'\n' || b == b'\0')
            .unwrap_or(slice.len());
        return Ok(String::from_utf8_lossy(&slice[..end])
            .trim_end_matches('/')
            .to_string());
    }

    Ok(raw.trim_end_matches('/').to_string())
}

/// Mengubah path relatif menjadi path aman di bawah `root`, menolak `..`
/// dan path absolut.
pub fn safe_join(root: &std::path::Path, entry: &str) -> Result<std::path::PathBuf> {
    use std::path::Component;

    let rel = std::path::Path::new(entry);
    let mut out = root.to_path_buf();
    for comp in rel.components() {
        match comp {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            Component::RootDir | Component::Prefix(_) | Component::ParentDir => {
                return Err(Error::UnsafePath(entry.to_string()))
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn member(name: &str, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(format!("{name:<16}").as_bytes());
        out.extend_from_slice(format!("{:<12}", 0).as_bytes());
        out.extend_from_slice(format!("{:<6}", 0).as_bytes());
        out.extend_from_slice(format!("{:<6}", 0).as_bytes());
        out.extend_from_slice(format!("{:<8}", 0o644).as_bytes());
        out.extend_from_slice(format!("{:<10}", data.len()).as_bytes());
        out.extend_from_slice(b"`\n");
        out.extend_from_slice(data);
        if data.len() % 2 == 1 {
            out.push(b'\n');
        }
        out
    }

    #[test]
    fn reads_plain_members_and_skips_padding() {
        let mut buf = MAGIC.to_vec();
        buf.extend(member("debian-binary", b"2.0\n"));
        buf.extend(member("control.tar.xz", b"odd"));
        let mut cur = Cursor::new(buf);
        let members = read_archive(&mut cur).unwrap();
        assert_eq!(members.len(), 2);
        assert_eq!(members[0].name, "debian-binary");
        assert_eq!(members[0].data, b"2.0\n");
        assert_eq!(members[1].data, b"odd");
    }

    #[test]
    fn resolves_gnu_long_names() {
        let mut long_names = Vec::new();
        let off = long_names.len();
        long_names.extend_from_slice(b"data.tar.xz/\n");
        let mut buf = MAGIC.to_vec();
        buf.extend(member("//", &long_names));
        buf.extend(member(&format!("/{off}"), b"xyz"));
        let members = read_archive(&mut Cursor::new(buf)).unwrap();
        assert_eq!(members[0].name, "data.tar.xz");
    }

    #[test]
    fn rejects_non_ar() {
        let mut cur = Cursor::new(b"not an ar".to_vec());
        assert!(matches!(read_archive(&mut cur), Err(Error::NotAr)));
    }

    #[test]
    fn safe_join_rejects_traversal() {
        let root = std::path::Path::new("/tmp/root");
        assert!(safe_join(root, "usr/bin/app").is_ok());
        assert!(safe_join(root, "../../etc/passwd").is_err());
        assert!(safe_join(root, "/etc/passwd").is_err());
    }
}
