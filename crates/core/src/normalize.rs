//! Normalisasi path Debian → tata letak Arch Linux.
//!
//! Arch menggabungkan `/bin`, `/sbin`, `/lib`, `/lib64` ke `/usr` dan tidak
//! memakai direktori multiarch (`/usr/lib/x86_64-linux-gnu`).

/// Apakah komponen direktori adalah triplet multiarch Debian, mis.
/// `x86_64-linux-gnu`, `arm-linux-gnueabihf`, `aarch64-linux-gnuabi64`.
pub fn is_multiarch_triplet(component: &str) -> bool {
    let Some(pos) = component.find("-linux-") else {
        return false;
    };
    let (arch, rest) = component.split_at(pos);
    if arch.is_empty() || arch.len() > 16 {
        return false;
    }
    if !arch.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return false;
    }
    let rest = rest.trim_start_matches("-linux-");
    rest.starts_with("gnu") || rest.starts_with("musl") || rest.starts_with("android")
}

/// Normalisasi satu path absolut (atau relatif tanpa awalan) ke tata letak
/// Arch. Mengembalikan path dengan awalan `/`.
pub fn normalize_path(path: &str) -> String {
    let trimmed = path.trim();
    let relative = trimmed.trim_start_matches('/');
    let parts: Vec<&str> = relative
        .split('/')
        .filter(|p| !p.is_empty() && *p != ".")
        .collect();

    if parts.is_empty() {
        return "/".to_string();
    }

    // /bin, /sbin, /lib, /lib32, /lib64 → /usr/...
    let mut remapped: Vec<&str> = match parts[0] {
        "bin" | "sbin" => {
            let mut head = vec!["usr", "bin"];
            head.extend_from_slice(&parts[1..]);
            head
        }
        "lib" | "lib32" | "lib64" => {
            let mut head = vec!["usr", "lib"];
            head.extend_from_slice(&parts[1..]);
            head
        }
        _ => parts.clone(),
    };

    // /usr/lib/<triplet>/... → /usr/lib/...
    if remapped.len() >= 3
        && remapped[0] == "usr"
        && remapped[1] == "lib"
        && is_multiarch_triplet(remapped[2])
    {
        remapped.remove(2);
    }

    format!("/{}", remapped.join("/"))
}

/// Normalisasi target symlink. Target relatif dibiarkan; target absolut
/// dinormalisasi agar menunjuk lokasi Arch yang benar.
pub fn normalize_link_target(target: &str) -> String {
    if target.starts_with('/') {
        normalize_path(target)
    } else {
        target.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_multiarch_lib() {
        assert_eq!(
            normalize_path("/usr/lib/x86_64-linux-gnu/libssl.so.3"),
            "/usr/lib/libssl.so.3"
        );
        assert_eq!(
            normalize_path("/usr/lib/aarch64-linux-gnu/gdk-pixbuf-2.0/2.10.0/loaders"),
            "/usr/lib/gdk-pixbuf-2.0/2.10.0/loaders"
        );
        assert_eq!(
            normalize_path("/usr/lib/arm-linux-gnueabihf/libc.so.6"),
            "/usr/lib/libc.so.6"
        );
    }

    #[test]
    fn merges_root_dirs_into_usr() {
        assert_eq!(normalize_path("/bin/bash"), "/usr/bin/bash");
        assert_eq!(normalize_path("/sbin/ip"), "/usr/bin/ip");
        assert_eq!(
            normalize_path("/lib/systemd/system/a.service"),
            "/usr/lib/systemd/system/a.service"
        );
        assert_eq!(
            normalize_path("/lib64/ld-linux-x86-64.so.2"),
            "/usr/lib/ld-linux-x86-64.so.2"
        );
    }

    #[test]
    fn keeps_regular_paths() {
        assert_eq!(
            normalize_path("/usr/share/applications/x.desktop"),
            "/usr/share/applications/x.desktop"
        );
        assert_eq!(normalize_path("usr/bin/x"), "/usr/bin/x");
        assert_eq!(normalize_path("./usr/bin/x"), "/usr/bin/x");
        assert_eq!(normalize_path("/etc/foo/bar.conf"), "/etc/foo/bar.conf");
    }

    #[test]
    fn does_not_mistake_plain_dirs_for_triplets() {
        assert!(!is_multiarch_triplet("pkgconfig"));
        assert!(!is_multiarch_triplet("x86_64-linux"));
        assert!(!is_multiarch_triplet("linux-gnu"));
        assert_eq!(
            normalize_path("/usr/lib/pkgconfig/foo.pc"),
            "/usr/lib/pkgconfig/foo.pc"
        );
    }

    #[test]
    fn rewrites_absolute_symlink_targets() {
        assert_eq!(
            normalize_link_target("/lib/x86_64-linux-gnu/libz.so.1"),
            "/usr/lib/libz.so.1"
        );
        assert_eq!(normalize_link_target("../share/x"), "../share/x");
    }
}
