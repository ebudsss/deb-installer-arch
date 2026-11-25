import { createContext, useContext } from "react";

export type Lang = "id" | "en";

const strings = {
  "app.name": ["DebInstaller", "DebInstaller"],
  "nav.home": ["Beranda", "Home"],
  "nav.installed": ["Terpasang", "Installed"],
  "nav.settings": ["Pengaturan", "Settings"],

  "home.title": ["Pasang paket Debian", "Install Debian packages"],
  "home.subtitle": [
    "Konversi .deb menjadi paket pacman native — terlacak, bisa dihapus bersih, dan tidak merusak sistem.",
    "Convert .deb into native pacman packages — tracked, cleanly removable, and safe for your system.",
  ],
  "home.drop": ["Jatuhkan file di sini", "Drop a file here"],
  "home.dropActive": ["Lepaskan untuk menganalisis", "Release to analyze"],
  "home.choose": ["Pilih file", "Choose a file"],
  "home.analyzing": ["Menganalisis paket…", "Analyzing package…"],
  "home.recent": ["Terakhir dibuka", "Recently opened"],
  "home.recentHint": ["Klik untuk membuka kembali", "Click to reopen"],
  "home.hint": [
    "Hanya file .deb. Konversi berjalan sebagai pengguna biasa; hanya instalasi meminta izin administrator.",
    "Only .deb files. Conversion runs as your user; only installation asks for administrator rights.",
  ],
  "home.formats": ["Format didukung", "Supported format"],
  "home.archBadge": ["Lingkungan Arch terdeteksi", "Arch environment detected"],
  "home.about.title": ["Bagaimana cara kerjanya", "How it works"],
  "home.about.format": ["Perbedaan format", "Format differences"],
  "home.about.formatBody": [
    "Debian memakai dpkg dan tata letak multiarch; Arch memakai pacman dengan /usr yang digabung.",
    "Debian uses dpkg with a multiarch layout; Arch uses pacman with a merged /usr.",
  ],
  "home.about.convert": ["Konversi paket", "Package conversion"],
  "home.about.convertBody": [
    "Isi data.tar dinormalisasi lalu dikemas ulang menjadi .pkg.tar.zst dengan metadata pacman.",
    "The data.tar payload is normalized and repacked as .pkg.tar.zst with pacman metadata.",
  ],
  "home.about.deps": ["Penanganan dependensi", "Dependency handling"],
  "home.about.depsBody": [
    "Nama Debian dipetakan ke paket Arch, dicek ke repo, pacman -F, dan AUR.",
    "Debian names are mapped to Arch packages, checked against repos, pacman -F, and the AUR.",
  ],
  "home.about.native": ["Manfaat paket native", "Native package benefits"],
  "home.about.nativeBody": [
    "Terdaftar di database pacman: bisa diperiksa, diperbarui, dan dihapus seperti paket biasa.",
    "Registered in the pacman database: inspectable, upgradable, and removable like any package.",
  ],

  "review.back": ["Kembali", "Back"],
  "review.install": ["Konversi & Pasang", "Convert & Install"],
  "review.blocked": [
    "Konversi ditolak: paket ini menimpa berkas sistem yang dilindungi.",
    "Conversion refused: this package overwrites protected system files.",
  ],
  "review.dangerAck": [
    "Saya sudah membaca script berisiko tinggi di atas dan tetap ingin melanjutkan.",
    "I have read the high-risk script above and want to continue anyway.",
  ],
  "review.dangerTitle": [
    "Script maintainer berisi perintah berisiko tinggi",
    "The maintainer script contains high-risk commands",
  ],
  "review.tab.info": ["Info", "Info"],
  "review.tab.deps": ["Dependensi", "Dependencies"],
  "review.tab.files": ["Berkas", "Files"],
  "review.tab.scripts": ["Script", "Scripts"],

  "info.package": ["Paket", "Package"],
  "info.version": ["Versi", "Version"],
  "info.arch": ["Arsitektur", "Architecture"],
  "info.description": ["Deskripsi", "Description"],
  "info.maintainer": ["Pemelihara", "Maintainer"],
  "info.homepage": ["Homepage", "Homepage"],
  "info.fileSize": ["Ukuran file", "File size"],
  "info.installedSize": ["Ukuran terpasang", "Installed size"],
  "info.fileCount": ["Jumlah berkas", "File count"],
  "info.sha256": ["SHA256", "SHA256"],
  "info.pkgname": ["Nama paket Arch", "Arch package name"],
  "info.dataMember": ["Anggota data", "Data member"],
  "info.alternatives": ["Sudah tersedia di Arch", "Already available on Arch"],
  "info.alternativesHint": [
    "Paket ini mungkin tidak perlu dikonversi. Pasang lewat pacman/pembantu AUR.",
    "You may not need to convert this package. Install it via pacman or an AUR helper.",
  ],
  "info.sensitive": ["Berkas sistem sensitif", "Sensitive system files"],

  "deps.debian": ["Debian", "Debian"],
  "deps.arch": ["Arch", "Arch"],
  "deps.status": ["Status", "Status"],
  "deps.note": ["Catatan", "Note"],
  "deps.summary": ["dependensi", "dependencies"],
  "deps.needsAttention": ["perlu perhatian", "need attention"],
  "deps.allClear": ["Semua dependensi terpenuhi.", "All dependencies are satisfied."],
  "deps.warnAur": [
    "Beberapa dependensi hanya ada di AUR. Pasang dulu sebelum melanjutkan.",
    "Some dependencies are AUR-only. Install them first.",
  ],
  "deps.warnMissing": [
    "Beberapa dependensi tidak ditemukan. Instalasi bisa gagal; tinjau daftar di bawah.",
    "Some dependencies were not found. Installation may fail; review the list below.",
  ],

  "files.path": ["Path terpasang", "Installed path"],
  "files.size": ["Ukuran", "Size"],
  "files.truncated": ["Daftar dipotong untuk tampilan.", "List truncated for display."],
  "files.empty": ["Paket tidak berisi berkas.", "The package contains no files."],
  "files.more": ["Tampilkan lebih banyak", "Show more"],

  "scripts.none": ["Tidak ada script maintainer.", "No maintainer scripts."],
  "scripts.risk": ["Risiko", "Risk"],
  "scripts.translated": ["Diterjemahkan ke hook pacman (.INSTALL)", "Translated to pacman hooks (.INSTALL)"],
  "scripts.advice": ["Saran", "Advice"],
  "scripts.noTranslation": [
    "Tidak ada perintah yang bisa diterjemahkan otomatis.",
    "No commands could be translated automatically.",
  ],

  "progress.title": ["Memproses paket", "Processing package"],
  "progress.stage.analyze": ["Analisis", "Analyze"],
  "progress.stage.convert": ["Konversi", "Convert"],
  "progress.stage.install": ["Instalasi", "Install"],
  "progress.stage.done": ["Selesai", "Done"],
  "progress.log": ["Log", "Log"],
  "progress.logEmpty": ["Belum ada keluaran.", "No output yet."],
  "progress.converting": ["Mengonversi .deb…", "Converting .deb…"],
  "progress.installing": ["Memasang dengan pacman (meminta izin)…", "Installing with pacman (asking for permission)…"],
  "progress.success": ["Paket berhasil dipasang.", "Package installed successfully."],
  "progress.failed": ["Instalasi gagal.", "Installation failed."],
  "progress.openFolder": ["Buka folder paket", "Open package folder"],
  "progress.done": ["Selesai", "Done"],
  "progress.retry": ["Coba lagi", "Try again"],

  "installed.title": ["Paket Terpasang", "Installed Packages"],
  "installed.subtitle": [
    "Kelola paket hasil konversi Debian yang terpasang di sistem.",
    "Manage Debian-converted packages installed on your system.",
  ],
  "installed.empty": ["Belum ada paket yang dikonversi.", "No packages converted yet."],
  "installed.emptyBody": [
    "Paket yang Anda konversi akan muncul di sini beserta status instalasinya.",
    "Packages you convert will appear here with their installation status.",
  ],
  "installed.emptyAction": ["Pasang paket pertama", "Install your first package"],
  "installed.stat.total": ["Total catatan", "Total records"],
  "installed.stat.installed": ["Terpasang", "Installed"],
  "installed.stat.pending": ["Belum terpasang", "Not installed"],
  "installed.stat.totalHint": ["Semua konversi", "All conversions"],
  "installed.stat.installedHint": ["Terdaftar di pacman", "Registered in pacman"],
  "installed.stat.pendingHint": ["Dikonversi, belum dipasang", "Converted, not installed"],
  "installed.search": ["Cari paket atau sumber…", "Search package or source…"],
  "installed.searchEmpty": ["Tidak ada paket yang cocok.", "No matching packages."],
  "installed.searchEmptyBody": ["Coba kata kunci lain.", "Try a different keyword."],
  "installed.confirmTitle": ["Hapus paket?", "Uninstall package?"],
  "installed.confirmBody": [
    "Paket dan berkasnya akan dihapus dari sistem dengan pacman -Rns. Tindakan ini tidak bisa dibatalkan.",
    "The package and its files will be removed from your system with pacman -Rns. This cannot be undone.",
  ],
  "installed.confirmWarning": [
    "Berkas konfigurasi di /etc akan disimpan sebagai .pacsave.",
    "Configuration files in /etc are kept as .pacsave.",
  ],
  "installed.uninstallDone": ["Paket dihapus.", "Package removed."],
  "installed.package": ["Paket", "Package"],
  "installed.version": ["Versi", "Version"],
  "installed.status": ["Status", "Status"],
  "installed.source": ["Sumber", "Source"],
  "installed.actions": ["", ""],
  "installed.installed": ["Terpasang", "Installed"],
  "installed.converted": ["Dikonversi", "Converted"],
  "installed.removed": ["Dihapus", "Removed"],
  "installed.uninstall": ["Hapus", "Uninstall"],
  "installed.open": ["Buka", "Open"],
  "installed.openHint": ["Buka aplikasi", "Open the app"],
  "installed.confirm": ["Yakin hapus?", "Confirm removal?"],
  "installed.yes": ["Ya, hapus", "Yes, remove"],
  "installed.cancel": ["Batal", "Cancel"],
  "installed.forget": ["Lupakan", "Forget"],
  "installed.busy": ["Menghapus…", "Removing…"],

  "settings.title": ["Pengaturan", "Settings"],
  "settings.subtitle": [
    "Preferensi tampilan dan perilaku konversi.",
    "Appearance and conversion preferences.",
  ],
  "settings.section.appearance": ["Tampilan", "Appearance"],
  "settings.section.packages": ["Manajemen paket", "Package management"],
  "settings.section.mapping": ["Pemetaan dependensi", "Dependency mapping"],
  "settings.section.about": ["Tentang aplikasi", "About"],
  "settings.about.version": ["Versi", "Version"],
  "settings.about.stack": ["Dibangun dengan", "Built with"],
  "settings.about.license": ["Lisensi", "License"],
  "settings.about.repo": ["Repositori", "Repository"],
  "settings.unsaved": ["Belum disimpan", "Unsaved changes"],
  "settings.localOnly": [
    "Disimpan di ~/.local/share/deb-installer",
    "Stored in ~/.local/share/deb-installer",
  ],
  "settings.privacy": [
    "Tidak ada telemetri. Permintaan jaringan hanya dilakukan ke AUR untuk memetakan dependensi, dan bisa dimatikan di atas.",
    "No telemetry. Network requests only go to the AUR for dependency mapping, and can be disabled above.",
  ],
  "settings.language": ["Bahasa", "Language"],
  "settings.theme": ["Tema", "Theme"],
  "settings.theme.system": ["Ikuti sistem", "Follow system"],
  "settings.theme.light": ["Terang", "Light"],
  "settings.theme.dark": ["Gelap", "Dark"],
  "settings.output": ["Folder paket hasil konversi", "Converted package folder"],
  "settings.choose": ["Pilih…", "Choose…"],
  "settings.default": ["Bawaan (cache aplikasi)", "Default (app cache)"],
  "settings.keep": ["Simpan .pkg.tar.zst setelah dipasang", "Keep .pkg.tar.zst after install"],
  "settings.aur": ["Periksa AUR saat memetakan dependensi", "Check AUR when mapping dependencies"],
  "settings.mapping": ["Mapping dependensi kustom (TOML)", "Custom dependency mapping (TOML)"],
  "settings.mappingHint": [
    "Contoh: [mapping] lalu \"libfoo1\" = \"foo\"",
    "Example: [mapping] then \"libfoo1\" = \"foo\"",
  ],
  "settings.save": ["Simpan", "Save"],
  "settings.saved": ["Tersimpan.", "Saved."],
  "settings.refreshDb": ["Perbarui database file pacman", "Refresh pacman file database"],
  "settings.refreshHint": [
    "Dibutuhkan agar pemetaan lewat nama library (pacman -F) akurat.",
    "Required for accurate library-name mapping (pacman -F).",
  ],
  "settings.refreshing": ["Memperbarui…", "Refreshing…"],

  "home.onlyDeb": [
    "Berkas tidak didukung. Gunakan .deb, .tar.gz, .tgz, .tar.xz, atau .tar.zst.",
    "Unsupported file. Use .deb, .tar.gz, .tgz, .tar.xz, or .tar.zst.",
  ],
  "home.archiveHint": [
    "Arsip tar dipasang ke /opt dengan tautan binari; .deb dikonversi penuh.",
    "Tar archives install into /opt with binary links; .deb is fully converted.",
  ],
  "archive.badge": ["Arsip tar", "Tar archive"],
  "archive.layout": ["Tata letak arsip", "Archive layout"],
  "archive.layout.system": [
    "Tata letak sistem (usr/, opt/, etc/)",
    "System layout (usr/, opt/, etc/)",
  ],
  "archive.layout.portable": [
    "Portable — dipasang ke /opt/{pkg}",
    "Portable — installed into /opt/{pkg}",
  ],
  "archive.executables": ["Binari yang ditautkan", "Linked binaries"],
  "archive.noExecutables": [
    "Tidak ada binari yang bisa ditautkan ke /usr/bin.",
    "No binaries could be linked into /usr/bin.",
  ],
  "archive.noDeps": [
    "Arsip tidak membawa metadata dependensi, jadi tidak ada yang dipetakan.",
    "Archives carry no dependency metadata, so nothing is mapped.",
  ],
  "archive.noScripts": [
    "Arsip tidak membawa script maintainer.",
    "Archives carry no maintainer scripts.",
  ],
  "home.about": [
    "Debian dan Arch Linux memakai tata letak berkas yang berbeda. DebInstaller menormalkan path, memetakan dependensi, dan memberi akhiran -deb pada nama paket agar tidak bentrok dengan paket repo.",
    "Debian and Arch Linux use different file layouts. DebInstaller normalizes paths, maps dependencies, and suffixes package names with -deb so they never collide with repo packages.",
  ],
  "settings.mappingAbout": [
    "Mapping kustom digabung di atas tabel bawaan. Format TOML: [mapping] berisi pasangan nama Debian ke nama paket Arch, dan [ignored] berisi daftar nama yang diabaikan.",
    "Custom mapping is merged on top of the built-in table. TOML format: [mapping] holds Debian-name to Arch-package pairs, and [ignored] lists names to skip.",
  ],
  "settings.runNow": ["Jalankan", "Run now"],
  "settings.refreshOk": ["Database file pacman diperbarui.", "pacman file database refreshed."],
  "settings.refreshFailed": [
    "Pembaruan dibatalkan atau gagal.",
    "Refresh was cancelled or failed.",
  ],
  "installed.removeFailed": ["Penghapusan gagal.", "Removal failed."],
  "installed.refresh": ["Muat ulang", "Reload"],
  "progress.diagnosis": ["Diagnosis", "Diagnosis"],
  "progress.files": ["berkas", "files"],
  "common.loading": ["Memuat…", "Loading…"],
  "common.retry": ["Coba lagi", "Retry"],
  "common.cancel": ["Batal", "Cancel"],
  "common.refresh": ["Muat ulang", "Refresh"],
  "footer.note": [
    "Konversi berjalan sebagai pengguna · hanya pacman yang memakai hak administrator",
    "Conversion runs as your user · only pacman uses administrator rights",
  ],
  "common.error": ["Terjadi kesalahan", "Something went wrong"],
  "common.close": ["Tutup", "Close"],
  "common.risk.safe": ["aman", "safe"],
  "common.risk.caution": ["perhatian", "caution"],
  "common.risk.dangerous": ["berbahaya", "dangerous"],
  "common.status.Installed": ["terpasang", "installed"],
  "common.status.Available": ["tersedia", "available"],
  "common.status.AurOnly": ["AUR", "AUR"],
  "common.status.Missing": ["tidak ditemukan", "not found"],
  "common.status.Ignored": ["diabaikan", "ignored"],
  "common.confidence.High": ["tinggi", "high"],
  "common.confidence.Medium": ["sedang", "medium"],
  "common.confidence.Low": ["rendah", "low"],
} as const satisfies Record<string, readonly [string, string]>;

export type StringKey = keyof typeof strings;

export function translate(lang: Lang, key: StringKey): string {
  const entry = strings[key];
  return lang === "en" ? entry[1] : entry[0];
}

export interface I18n {
  lang: Lang;
  t: (key: StringKey) => string;
}

/** Bahasa Inggris adalah bawaan; Indonesia tersedia lewat Pengaturan. */
export const I18nContext = createContext<I18n>({
  lang: "en",
  t: (key) => translate("en", key),
});

export function useI18n(): I18n {
  return useContext(I18nContext);
}
