# DebInstaller

GUI installer `.deb` untuk Arch Linux (dan turunannya seperti CachyOS/Manjaro).

Paket `.deb` **tidak pernah** dipasang dengan `dpkg`. Alih-alih, DebInstaller
mengonversinya menjadi paket pacman native (`.pkg.tar.zst`) lalu memasangnya
dengan `pacman -U`. Hasilnya: paket terlacak di database pacman, bisa dihapus
bersih, dan tidak merusak sistem.

## Fitur

- Drag-and-drop `.deb`, atau buka lewat `deb-installer file.deb` / klik kanan di file manager.
- Metadata lengkap: nama, versi, arsitektur, deskripsi, ukuran, SHA256.
- Pemetaan dependensi Debian → Arch dengan empat strategi: tabel bawaan,
  `pacman -F`, pencocokan nama, dan saran AUR. Setiap hasil diberi status dan
  tingkat keyakinan.
- Deteksi paket yang sudah tersedia di repo/AUR sebelum konversi.
- Normalisasi path: `/usr/lib/<triplet>/` → `/usr/lib/`, `/bin|/sbin|/lib|/lib64` → `/usr/...`.
- Analisis script maintainer (`preinst`, `postinst`, `prerm`, `postrm`) dengan
  tingkat risiko, plus terjemahan perintah umum ke hook `.INSTALL`.
- Penolakan paket yang menimpa berkas sistem yang dilindungi
  (`/var/lib/pacman`, `/etc/pacman.conf`, `/usr/bin/pacman`, ...).
- Instalasi via `pkexec pacman -U` (polkit) dengan log real-time dan diagnosis
  kegagalan yang bisa ditindaklanjuti.
- Riwayat konversi dan tombol hapus paket.
- Antarmuka dua bahasa (Indonesia/English), tema terang/gelap.

## Arsitektur

```
src/            React + Tailwind (UI)
src-tauri/      Tauri 2 backend (perintah IPC)
crates/core/    Library inti: ar, control, deb, deps, scripts, normalize, pkgbuild, mtree, install
crates/cli/     CLI `deb-installer-cli` untuk pengujian dan otomasi
packaging/      PKGBUILD, .desktop, MIME
```

Alur: `deb_parser` → `dep_mapper` → `script_analyzer` → `pkg_builder` → `installer`.

## Membangun

Prasyarat: Rust, Node.js + pnpm, `webkit2gtk-4.1`, `gtk3`, `polkit`, `pacman`,
`zstd`, `xz`, `curl`.

```sh
pnpm install
pnpm build                      # frontend → dist/
cargo build --release -p deb-installer --features custom-protocol
```

Untuk pengembangan dengan hot reload:

```sh
pnpm dev                        # terminal 1
cargo tauri dev                 # terminal 2 (butuh tauri-cli)
```

## Menjalankan

```sh
./run.sh                    # tanpa argumen
./run.sh file.deb           # langsung membuka tinjauan paket itu
```

### Memasang ke menu aplikasi

```sh
./packaging/install-local.sh              # pasang
./packaging/install-local.sh --uninstall  # cabut
```

Dipasang tanpa root ke direktori pengguna:

| Berkas | Lokasi |
|---|---|
| Binari rilis | `~/.local/bin/deb-installer` |
| Entri menu | `~/.local/share/applications/deb-installer.desktop` |
| Ikon 32/128/256/512 px | `~/.local/share/icons/hicolor/*/apps/` |
| Asosiasi `.deb` | `~/.local/share/mime/packages/deb-installer.xml` |

Setelah itu **DebInstaller** muncul di menu aplikasi, dan klik kanan file `.deb`
menawarkan "Buka dengan DebInstaller". Entri menu memakai
`env WEBKIT_DISABLE_DMABUF_RENDERER=1` karena `Exec=` tidak bisa memuat
penugasan variabel langsung.

`run.sh` hanya membungkus binari di `target/debug/` dan menyalakan
`WEBKIT_DISABLE_DMABUF_RENDERER=1`. Variabel itu wajib di sebagian sistem
(NVIDIA proprietary, atau Mesa tanpa DRI3): tanpanya WebKitGTK gagal membuat
GBM buffer, jendela terbuka tetapi isinya kosong. Bila tampilan Anda normal,
variabel itu boleh dimatikan.

Untuk build rilis yang berdiri sendiri:

```sh
pnpm build
cargo build --release -p deb-installer --features custom-protocol
./target/release/deb-installer file.deb
```

## CLI

```sh
deb-installer-cli info     paket.deb
deb-installer-cli files    paket.deb
deb-installer-cli deps     paket.deb --no-network
deb-installer-cli scripts  paket.deb
deb-installer-cli build    paket.deb -o ./out
deb-installer-cli install  ./out/paket-deb-1.0-1-x86_64.pkg.tar.zst
deb-installer-cli uninstall paket-deb
deb-installer-cli history
```

Semua subperintah menerima `--json` kecuali `files`, `scripts`, dan `install`/`uninstall`.

## Kinerja konversi

Angka pada `omniget_1.0.1_amd64.deb` (137 MB, payload 295 MB, 207 berkas, 16 core):

| Tahap | Sebelum | Sesudah |
|---|---|---|
| Baca + SHA256 | 2,0 s | 0,08 s |
| Pindai `data.tar` | 3,6 s | 0,75 s |
| Petakan dependensi (AUR) | 3,5 s | 1,5 s |
| Bangun paket (zstd) | >180 s | 3,2 s |
| **Total** | **±190 s** | **±5,5 s** |

Terverifikasi end-to-end di mesin 16 core: `omniget-deb 1.0.1-1` terpasang,
`pacman -Qkk` melaporkan **0 berkas berubah** dari 207, dan `ldd` tidak
menemukan library yang hilang.

Tiga penyebab dan perbaikannya:

1. **Debug build mengompilasi semua dependency dengan `-O0`**, termasuk kode C
   `zstd-sys`. `[profile.dev.package."*"] opt-level = 3` di `Cargo.toml` membuat
   build debug tetap cepat tanpa mengorbankan waktu kompilasi kode sendiri.
2. **zstd level 19 satu thread** butuh 36 detik untuk payload 295 MB. Encoder kini
   memakai `multithread` (maks. 8 worker) dan level bawaan 10 — 3 detik untuk
   118 MB, dibanding 20 detik untuk 107 MB di level 19.
3. **Query AUR per dependensi** (≈1,2 detik masing-masing). Semua kandidat
   dikumpulkan dulu lalu diambil dalam satu permintaan RPC.

Kemajuan tiap tahap dilaporkan ke UI lewat event `progress`, jadi layar tidak
lagi terlihat menggantung selama kompresi.

## Pengujian

```sh
cargo test -p deb-installer-core
```

Unit test mencakup parser `ar`/`control`, normalisasi path, pemetaan dependensi,
analisis script, konversi versi, mtree, diagnosis pacman, serta riwayat.
`crates/core/tests/pipeline.rs` menguji alur penuh pada `.deb` sintetis.

Uji instalasi nyata sebaiknya dilakukan di container/VM Arch:

```sh
docker run --rm -it -v "$PWD:/src:ro" archlinux:latest bash
# di dalam container:
pacman -Sy --noconfirm
# bangun paket dengan CLI, lalu:
pacman -U --noconfirm paket-deb-1.0-1-x86_64.pkg.tar.zst
pacman -Qkk paket-deb
pacman -Rns --noconfirm paket-deb
```

## Keamanan

- Script maintainer tidak pernah dieksekusi. Hanya perintah yang punya padanan
  aman di Arch yang diterjemahkan ke `.INSTALL`; sisanya ditampilkan sebagai
  temuan dengan saran.
- Script berisiko tinggi (`curl|sh`, `dpkg`, `debconf`) mewajibkan centang
  persetujuan eksplisit sebelum tombol instalasi aktif.
- Hanya satu langkah berjalan sebagai root: `pacman -U`/`pacman -Rns` lewat
  `pkexec`. Konversi berjalan sebagai pengguna biasa.
- Paket yang menyentuh database pacman atau berkas kritis lain ditolak.
- SHA256 file `.deb` ditampilkan untuk verifikasi manual.
- Tidak ada telemetri.

## Lisensi

MIT.
