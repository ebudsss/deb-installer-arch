//! Parser file `control` Debian dan sintaks relasi dependensi.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Hasil parse file `control` (RFC822-ish, field satu per baris, baris
/// lanjutan diawali spasi).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Control {
    /// Semua field apa adanya, kunci huruf kecil.
    pub fields: BTreeMap<String, String>,
}

impl Control {
    pub fn parse(text: &str) -> Result<Self> {
        let mut fields: BTreeMap<String, String> = BTreeMap::new();
        let mut current: Option<String> = None;

        for line in text.lines() {
            if line.trim().is_empty() {
                // Baris kosong memisahkan paragraf; paragraf kedua pada
                // control.tar biasanya berupa signature (md5sums), bukan control.
                break;
            }
            if line.starts_with(' ') || line.starts_with('\t') {
                let key = current
                    .as_ref()
                    .ok_or_else(|| Error::BadControl("baris lanjutan tanpa field".into()))?;
                let value = fields.get_mut(key).expect("field aktif");
                value.push('\n');
                value.push_str(line.trim_end());
                continue;
            }
            let (key, value) = line
                .split_once(':')
                .ok_or_else(|| Error::BadControl(format!("baris tanpa `:`: {line}")))?;
            let key = key.trim().to_ascii_lowercase();
            if key.is_empty() {
                return Err(Error::BadControl(format!("nama field kosong: {line}")));
            }
            if fields.contains_key(&key) {
                return Err(Error::BadControl(format!("field ganda: {key}")));
            }
            fields.insert(key.clone(), value.trim().to_string());
            current = Some(key);
        }

        Ok(Self { fields })
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields
            .get(&key.to_ascii_lowercase())
            .map(String::as_str)
    }

    pub fn require(&self, key: &'static str) -> Result<&str> {
        self.get(key).ok_or(Error::MissingField(key))
    }

    pub fn package(&self) -> Result<&str> {
        self.require("package")
    }

    pub fn version(&self) -> Result<&str> {
        self.require("version")
    }

    pub fn architecture(&self) -> Result<&str> {
        self.require("architecture")
    }

    pub fn description(&self) -> &str {
        self.get("description").unwrap_or("")
    }

    /// Deskripsi satu baris: baris pertama sinopsis.
    pub fn synopsis(&self) -> &str {
        self.description().lines().next().unwrap_or("").trim()
    }

    pub fn maintainer(&self) -> &str {
        self.get("maintainer").unwrap_or("")
    }

    pub fn depends(&self) -> Vec<Relationship> {
        self.relationships("depends")
    }

    pub fn pre_depends(&self) -> Vec<Relationship> {
        self.relationships("pre-depends")
    }

    pub fn recommends(&self) -> Vec<Relationship> {
        self.relationships("recommends")
    }

    pub fn conflicts(&self) -> Vec<Relationship> {
        self.relationships("conflicts")
    }

    pub fn provides(&self) -> Vec<Relationship> {
        self.relationships("provides")
    }

    fn relationships(&self, field: &str) -> Vec<Relationship> {
        self.get(field)
            .map(Relationship::parse_list)
            .unwrap_or_default()
    }
}

/// Operator versi Debian.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VersionOp {
    Lt,
    Le,
    Eq,
    Ge,
    Gt,
}

impl VersionOp {
    pub fn as_str(self) -> &'static str {
        match self {
            VersionOp::Lt => "<<",
            VersionOp::Le => "<=",
            VersionOp::Eq => "=",
            VersionOp::Ge => ">=",
            VersionOp::Gt => ">>",
        }
    }
}

/// Satu dependensi dengan versi opsional.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dependency {
    pub name: String,
    pub op: Option<VersionOp>,
    pub version: Option<String>,
    pub arch_qualifiers: Vec<String>,
}

impl Dependency {
    pub fn parse(input: &str) -> Option<Self> {
        let input = input.trim();
        if input.is_empty() {
            return None;
        }
        // Buang profil build: `foo <!nocheck>`.
        let mut rest = input;
        if let Some(idx) = rest.find('<') {
            rest = rest[..idx].trim();
        }
        // Buang qualifier arsitektur: `foo:any` / `foo [amd64]`.
        let mut arch_qualifiers = Vec::new();
        if let Some(start) = rest.find('[') {
            if let Some(end) = rest.find(']') {
                arch_qualifiers = rest[start + 1..end]
                    .split_whitespace()
                    .map(|s| s.trim_start_matches('!').to_string())
                    .collect();
            }
            rest = rest[..start].trim();
        }

        let (name, version_part) = match rest.find('(') {
            Some(start) => {
                let end = rest.find(')')?;
                let name = rest[..start].trim();
                let inner = rest[start + 1..end].trim();
                (name, Some(inner))
            }
            None => (rest, None),
        };

        let name = name.split(':').next().unwrap_or(name).trim().to_string();
        if name.is_empty() {
            return None;
        }

        let (op, version) = match version_part {
            Some(v) => {
                let mut parts = v.splitn(2, char::is_whitespace);
                let op_str = parts.next()?.trim();
                let ver = parts.next()?.trim().to_string();
                let op = match op_str {
                    "<<" => VersionOp::Lt,
                    "<=" => VersionOp::Le,
                    "=" => VersionOp::Eq,
                    ">=" => VersionOp::Ge,
                    ">>" => VersionOp::Gt,
                    // `<` dan `>` usang, diperlakukan seperti `<=`/`>=`.
                    "<" => VersionOp::Le,
                    ">" => VersionOp::Ge,
                    _ => return None,
                };
                (Some(op), Some(ver))
            }
            None => (None, None),
        };

        Some(Self {
            name,
            op,
            version,
            arch_qualifiers,
        })
    }
}

/// Satu klausa relasi: alternatif dipisah `|`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relationship {
    pub alternatives: Vec<Dependency>,
}

impl Relationship {
    pub fn parse_list(value: &str) -> Vec<Self> {
        value
            .split(',')
            .filter_map(|clause| {
                let alternatives: Vec<Dependency> =
                    clause.split('|').filter_map(Dependency::parse).collect();
                if alternatives.is_empty() {
                    None
                } else {
                    Some(Relationship { alternatives })
                }
            })
            .collect()
    }

    pub fn primary(&self) -> &Dependency {
        &self.alternatives[0]
    }

    pub fn to_debian_string(&self) -> String {
        self.alternatives
            .iter()
            .map(|d| match (&d.op, &d.version) {
                (Some(op), Some(v)) => format!("{} ({} {})", d.name, op.as_str(), v),
                _ => d.name.clone(),
            })
            .collect::<Vec<_>>()
            .join(" | ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONTROL: &str = "Package: vscode\nVersion: 1.90.0-1\nArchitecture: amd64\nMaintainer: Microsoft <support@example.com>\nDepends: libc6 (>= 2.28), libgtk-3-0 | libgtk-4-1, libnss3\nPre-Depends: dpkg (>= 1.14)\nDescription: Code editing. Redefined.\n Visual Studio Code is a code editor.\n .\n It has lots of features.\n";

    #[test]
    fn parses_fields_and_multiline_description() {
        let c = Control::parse(CONTROL).unwrap();
        assert_eq!(c.package().unwrap(), "vscode");
        assert_eq!(c.version().unwrap(), "1.90.0-1");
        assert_eq!(c.architecture().unwrap(), "amd64");
        assert_eq!(c.synopsis(), "Code editing. Redefined.");
        assert!(c.description().contains("lots of features"));
        assert!(c.maintainer().contains("Microsoft"));
    }

    #[test]
    fn parses_dependency_alternatives() {
        let c = Control::parse(CONTROL).unwrap();
        let deps = c.depends();
        assert_eq!(deps.len(), 3);
        assert_eq!(deps[0].primary().name, "libc6");
        assert_eq!(deps[0].primary().op, Some(VersionOp::Ge));
        assert_eq!(deps[0].primary().version.as_deref(), Some("2.28"));
        assert_eq!(deps[1].alternatives.len(), 2);
        assert_eq!(deps[1].alternatives[1].name, "libgtk-4-1");
        assert_eq!(deps[2].primary().name, "libnss3");
        assert_eq!(deps[2].primary().op, None);
    }

    #[test]
    fn strips_arch_qualifiers_and_profiles() {
        let d = Dependency::parse("libfoo:any (>= 1.0) [amd64] <!nocheck>").unwrap();
        assert_eq!(d.name, "libfoo");
        assert_eq!(d.arch_qualifiers, vec!["amd64".to_string()]);
        assert_eq!(d.op, Some(VersionOp::Ge));
    }

    #[test]
    fn rejects_duplicate_field() {
        let err = Control::parse("Package: a\nPackage: b\n").unwrap_err();
        assert!(matches!(err, Error::BadControl(_)));
    }

    #[test]
    fn stops_at_blank_line() {
        let c = Control::parse("Package: a\n\nbogus trailing").unwrap();
        assert_eq!(c.package().unwrap(), "a");
        assert_eq!(c.fields.len(), 1);
    }
}
