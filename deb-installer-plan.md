# Rencana Proyek: GUI Installer `.deb` untuk Arch Linux

## 1. Ringkasan

Aplikasi desktop GUI yang memungkinkan pengguna Arch Linux (termasuk turunannya seperti CachyOS dan Manjaro) memasang file `.deb` dengan aman. Aplikasi mengonversi `.deb` menjadi paket pacman native, sehingga file tetap terlacak, bisa di-uninstall bersih, dan tidak merusak sistem.

**Prinsip utama:** tidak pernah memakai `dpkg` langsung. Selalu konversi ke paket pacman lalu `pacman -U`.

## 2. Tujuan dan Non-Tujuan

**Tujuan**
- Drag-and-drop `.deb` lalu klik Install.
- Pemetaan dependensi Debian → Arch otomatis dan transparan.
- Tampilkan script maintainer (`postinst`, dll.) sebelum dijalankan.
- Paket hasil konversi tercatat di pacman dan bisa dihapus dari GUI.
- Integrasi file manager: klik kanan → "Install dengan DebInstaller".

**Non-Tujuan (versi awal)**
- Menggantikan AUR/Flatpak/AppImage.
- Mendukung distro non-Arch.
- Menjamin semua `.deb` berjalan (beberapa memang tidak kompatibel).

## 3. Pengguna Target

Pengguna desktop Arch/CachyOS yang menemukan aplikasi hanya tersedia dalam `.deb` dan ingin cara yang lebih aman dan mudah dibanding `debtap` di terminal.

## 4. Fitur

### MVP
1. Pilih/drag-drop file `.deb`.
2. Baca metadata: nama, versi, arsitektur, deskripsi, ukuran, dependensi.
3. Cek apakah paket serupa sudah ada di repo resmi/AUR (saran alternatif).
4. Petakan dependensi dan tampilkan status (tersedia, tidak ditemukan, perlu dicek manual).
5. Tampilkan daftar file yang akan terpasang dan script maintainer.
6. Konversi ke `.pkg.tar.zst`.
7. Install via `pacman -U` dengan otorisasi `polkit` (`pkexec`).
8. Log proses real-time.

### Pasca-MVP
- Daftar paket hasil konversi + tombol uninstall.
- Riwayat instalasi.
- Mapping dependensi yang bisa diedit user dan disimpan.
- Integrasi file manager (`.desktop` + MIME `application/vnd.debian.binary-package`).
- Deteksi library bermasalah (misal `libssl1.1`) dan saran solusi.
- Opsi "ekstrak ke folder sendiri" (mode portable di `~/.local/opt`).
- Pengaturan: tema, bahasa (ID/EN), direktori cache.

## 5. Alur Pengguna

```
Buka .deb → Analisis → Review (dependensi, file, script) → Konfirmasi
→ Konversi → Autentikasi (polkit) → pacman -U → Selesai / Error + saran
```

## 6. Arsitektur

```
┌──────────────────────────────┐
│  Frontend (UI, web stack)    │
└──────────────┬───────────────┘
               │ IPC (Tauri commands)
┌──────────────▼───────────────┐
│  Core (Rust)                 │
│  ├─ deb_parser               │
│  ├─ dep_mapper               │
│  ├─ script_analyzer          │
│  ├─ pkg_builder              │
│  └─ installer (pacman/polkit)│
└──────────────┬───────────────┘
               │
   pacman · pacman -F · AUR RPC · makepkg/bsdtar
```

### Stack yang direkomendasikan
- **Tauri 2** (Rust backend + web frontend): ringan, sesuai kalau sudah nyaman dengan UI berbasis web.
- **Frontend:** React/Svelte + Tailwind.
- **Parsing:** crate `ar`, `tar`, `zstd`/`xz2`/`flate2`.
- **Privilege:** `pkexec` (polkit), hanya untuk langkah `pacman -U`. Konversi berjalan sebagai user biasa.

## 7. Modul Inti

### 7.1 `deb_parser`
- Buka arsip `ar`, ekstrak `control.tar.*` dan `data.tar.*` (gz/xz/zst).
- Parse file `control` (Package, Version, Architecture, Depends, Pre-Depends, Description, Maintainer).
- Daftar isi `data.tar` tanpa ekstraksi penuh.

### 7.2 `dep_mapper`
Urutan strategi pemetaan:
1. **Tabel mapping lokal** (`mapping.toml`) untuk nama umum, contoh `libgtk-3-0 = "gtk3"`.
2. **`pacman -F`** untuk mencari paket Arch yang menyediakan file `.so` terkait.
3. **Pencocokan nama** langsung (`libfoo` → `foo`).
4. **Tidak ditemukan:** tandai dan minta keputusan user (abaikan / pilih manual).

Output per dependensi: `{debian_name, arch_name, status, confidence}`.

### 7.3 `script_analyzer`
- Ambil `preinst`, `postinst`, `prerm`, `postrm`.
- Deteksi perintah khas Debian: `dpkg-divert`, `update-alternatives`, `debconf`, `invoke-rc.d`, `deb-systemd-helper`.
- Beri level risiko (aman / perhatian / berbahaya) dan tampilkan ke user.
- Terjemahkan yang umum ke hook pacman (`.INSTALL`), selebihnya hanya peringatan.

### 7.4 `pkg_builder`
- Ekstrak `data.tar` ke direktori staging.
- Normalisasi path: `/usr/lib/x86_64-linux-gnu/` → `/usr/lib/`; pindahkan `/lib`, `/bin`, `/sbin` ke `/usr/...` (Arch menggabungkan lokasi ini).
- Buat `.PKGINFO` (atau `PKGBUILD` otomatis + `makepkg`).
- Kemas jadi `.pkg.tar.zst`.
- Beri nama paket dengan suffix, misalnya `nama-deb`, untuk menghindari bentrok dengan paket repo.

### 7.5 `installer`
- Jalankan `pkexec pacman -U <file>`.
- Stream stdout/stderr ke UI.
- Tangani error umum (konflik file, dependensi hilang, kunci database pacman).

## 8. Keamanan

- Jangan eksekusi script maintainer tanpa persetujuan eksplisit user.
- Hanya satu langkah berprivilege (`pacman -U`), sisanya sebagai user biasa.
- Tolak/waspadai `.deb` yang menimpa file milik paket lain atau menyentuh path sensitif (`/etc/pacman.d`, `/boot`, `/usr/bin/pacman`).
- Tampilkan checksum SHA256 file `.deb` untuk verifikasi manual.
- Peringatan bahwa `.deb` dari sumber tidak dikenal berisiko.
- Tidak ada telemetri tanpa persetujuan.

## 9. Struktur Proyek

```
deb-installer/
├─ src-tauri/
│  ├─ src/
│  │  ├─ main.rs
│  │  ├─ deb_parser.rs
│  │  ├─ dep_mapper.rs
│  │  ├─ script_analyzer.rs
│  │  ├─ pkg_builder.rs
│  │  └─ installer.rs
│  ├─ data/mapping.toml
│  └─ Cargo.toml
├─ src/                  # frontend
│  ├─ pages/ (Home, Review, Progress, Installed, Settings)
│  └─ components/
├─ packaging/
│  ├─ PKGBUILD           # untuk rilis di AUR
│  └─ deb-installer.desktop
└─ docs/
```

## 10. Rancangan UI

| Layar | Isi |
|---|---|
| **Home** | Area drag-and-drop, tombol pilih file, daftar terakhir |
| **Review** | Tab: Info, Dependensi, File, Script. Tombol Install / Batal |
| **Progress** | Langkah (Analisis → Konversi → Install) + log |
| **Installed** | Daftar paket hasil konversi, uninstall |
| **Settings** | Bahasa, tema, cache, mapping kustom |

Catatan desain: tampilkan peringatan risiko dengan warna jelas dan bahasa sederhana, bukan istilah teknis semata.

## 11. Roadmap

| Fase | Target | Estimasi |
|---|---|---|
| 0 | Riset, kumpulkan 10-20 `.deb` populer untuk uji (Discord, VS Code, Chrome, Zoom, dll.) | 2-3 hari |
| 1 | `deb_parser` + CLI internal untuk uji | 1 minggu |
| 2 | `dep_mapper` + tabel mapping awal | 1-2 minggu |
| 3 | `pkg_builder` + `installer` (`pacman -U` via polkit) | 1-2 minggu |
| 4 | UI Tauri: Home, Review, Progress | 1-2 minggu |
| 5 | `script_analyzer`, halaman Installed, uninstall | 1-2 minggu |
| 6 | Integrasi file manager, packaging AUR, beta publik | 1 minggu |

## 12. Pengujian

- **Unit test:** parser `control`, pemetaan nama, normalisasi path.
- **Korpus uji:** kumpulan `.deb` nyata dengan hasil yang diharapkan (berhasil / gagal / butuh peringatan).
- **VM/container Arch bersih** untuk uji instalasi (hindari mengotori sistem utama).
- **Uji uninstall:** pastikan tidak ada file sisa.

## 13. Risiko dan Mitigasi

| Risiko | Mitigasi |
|---|---|
| Pemetaan dependensi meleset | Kombinasi tabel + `pacman -F`, tandai confidence, user bisa edit |
| Versi library tidak cocok (misal `libssl1.1`) | Deteksi dan beri saran (paket `-compat` di AUR) |
| Script maintainer berbahaya | Wajib review, level risiko, tidak dieksekusi otomatis |
| Bentrok dengan paket repo | Suffix nama + cek `conflicts` |
| Fitur lebih rumit dari manfaatnya | Arahkan ke AUR/Flatpak/AppImage bila sudah tersedia |

## 14. Distribusi

- Rilis lewat **AUR** (`deb-installer` / `deb-installer-git`).
- Opsional: AppImage dan Flatpak (catatan: Flatpak sandbox menyulitkan akses `pacman`).
- Repo GitHub + lisensi open source (MIT/GPL-3.0).

## 15. Pertanyaan Terbuka

1. Nama aplikasi?
2. Konversi via `.PKGINFO` langsung atau `PKGBUILD` + `makepkg`? (`PKGBUILD` lebih standar, `.PKGINFO` langsung lebih cepat)
3. Perlu dukungan multi-bahasa sejak awal?
4. Lisensi apa yang dipakai?

## 16. Langkah Berikutnya

1. Setup proyek Tauri dan modul `deb_parser`.
2. Kumpulkan korpus `.deb` uji.
3. Buat `mapping.toml` awal.
4. Buat wireframe 3 layar utama (Home, Review, Progress).
