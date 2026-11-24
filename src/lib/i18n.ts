import { createContext, useContext } from "react";

export type Lang = "id" | "en";

const strings = {
  "app.name": ["DebInstaller", "DebInstaller"],
  "nav.home": ["Beranda", "Home"],
  "nav.installed": ["Terpasang", "Installed"],
  "nav.settings": ["Pengaturan", "Settings"],

  "home.title": ["Pasang paket .deb dengan aman", "Install .deb packages safely"],
  "home.subtitle": [
    "Paket dikonversi menjadi paket pacman native, jadi tetap terlacak dan bisa dihapus bersih.",
    "Packages are converted to native pacman packages, so they stay tracked and uninstall cleanly.",
  ],
  "home.drop": ["Jatuhkan file .deb di sini", "Drop a .deb file here"],
  "home.choose": ["Pilih file .deb", "Choose a .deb file"],
  "home.analyzing": ["Menganalisis paket…", "Analyzing package…"],
  "home.recent": ["Terakhir dibuka", "Recently opened"],
  "home.hint": [
    "Hanya file .deb. Konversi berjalan sebagai pengguna biasa; hanya instalasi meminta izin administrator.",
    "Only .deb files. Conversion runs as your user; only installation asks for administrator rights.",
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
  "progress.converting": ["Mengonversi .deb…", "Converting .deb…"],
  "progress.installing": ["Memasang dengan pacman (meminta izin)…", "Installing with pacman (asking for permission)…"],
  "progress.success": ["Paket berhasil dipasang.", "Package installed successfully."],
  "progress.failed": ["Instalasi gagal.", "Installation failed."],
  "progress.openFolder": ["Buka folder paket", "Open package folder"],
  "progress.done": ["Selesai", "Done"],
  "progress.retry": ["Coba lagi", "Try again"],

  "installed.title": ["Paket hasil konversi", "Converted packages"],
  "installed.empty": ["Belum ada paket yang dikonversi.", "No packages converted yet."],
  "installed.package": ["Paket", "Package"],
  "installed.version": ["Versi", "Version"],
  "installed.status": ["Status", "Status"],
  "installed.source": ["Sumber", "Source"],
  "installed.actions": ["", ""],
  "installed.installed": ["Terpasang", "Installed"],
  "installed.converted": ["Dikonversi", "Converted"],
  "installed.removed": ["Dihapus", "Removed"],
  "installed.uninstall": ["Hapus", "Uninstall"],
  "installed.confirm": ["Yakin hapus?", "Confirm removal?"],
  "installed.yes": ["Ya, hapus", "Yes, remove"],
  "installed.cancel": ["Batal", "Cancel"],
  "installed.forget": ["Lupakan", "Forget"],
  "installed.busy": ["Menghapus…", "Removing…"],

  "settings.title": ["Pengaturan", "Settings"],
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
    "Hanya berkas .deb yang didukung.",
    "Only .deb files are supported.",
  ],
  "home.about": [
    "Debian dan Arch Linux memakai tata letak berkas yang berbeda. DebInstaller menormalkan path, memetakan dependensi, dan memberi akhiran -deb pada nama paket agar tidak bentrok dengan paket repo.",
    "Debian and Arch Linux use different file layouts. DebInstaller normalizes paths, maps dependencies, and suffixes package names with -deb so they never collide with repo packages.",
  ],
  "settings.mappingAbout": [
    "Mapping kustom digabung di atas tabel bawaan. Format TOML: [mapping] berisi pasangan nama Debian ke nama paket Arch, dan [ignored] berisi daftar nama yang diabaikan.",
    "Custom mapping is merged on top of the built-in table. TOML format: [mapping] holds Debian-name to Arch-package pairs, and [ignored] lists names to skip.",
  ],
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

export const I18nContext = createContext<I18n>({
  lang: "id",
  t: (key) => translate("id", key),
});

export function useI18n(): I18n {
  return useContext(I18nContext);
}
