#!/bin/sh
# Memasang DebInstaller ke menu aplikasi untuk pengguna saat ini
# (tanpa root, tanpa menyentuh /usr).
#
# Yang dipasang:
#   ~/.local/bin/deb-installer                          binari rilis
#   ~/.local/share/applications/deb-installer.desktop   entri menu
#   ~/.local/share/icons/hicolor/*/apps/...             ikon
#   ~/.local/share/mime/packages/deb-installer.xml      asosiasi file .deb
#
# Jalankan `packaging/install-local.sh --uninstall` untuk membatalkan.
set -eu
cd "$(dirname "$0")/.."

BIN_SRC="target/release/deb-installer"
BIN_DST="$HOME/.local/bin/deb-installer"
APPS="$HOME/.local/share/applications"
ICONS="$HOME/.local/share/icons/hicolor"
MIME="$HOME/.local/share/mime/packages"

uninstall() {
  rm -f "$BIN_DST" "$APPS/deb-installer.desktop" "$MIME/deb-installer.xml"
  for size in 32x32 128x128 256x256 512x512; do
    rm -f "$ICONS/$size/apps/deb-installer.png"
  done
  update-desktop-database "$APPS" 2>/dev/null || true
  update-mime-database "$HOME/.local/share/mime" 2>/dev/null || true
  gtk-update-icon-cache -q -t -f "$ICONS" 2>/dev/null || true
  echo "DebInstaller dihapus dari menu aplikasi."
}

if [ "${1:-}" = "--uninstall" ]; then
  uninstall
  exit 0
fi

if [ ! -x "$BIN_SRC" ]; then
  echo "Binari rilis belum ada. Jalankan dulu:" >&2
  echo "  pnpm build && cargo build --release -p deb-installer --features custom-protocol" >&2
  exit 1
fi

mkdir -p "$(dirname "$BIN_DST")" "$APPS" "$MIME" \
  "$ICONS/32x32/apps" "$ICONS/128x128/apps" "$ICONS/256x256/apps" "$ICONS/512x512/apps"

install -m755 "$BIN_SRC" "$BIN_DST"
install -m644 src-tauri/icons/32x32.png "$ICONS/32x32/apps/deb-installer.png"
install -m644 src-tauri/icons/128x128.png "$ICONS/128x128/apps/deb-installer.png"
install -m644 src-tauri/icons/128x128@2x.png "$ICONS/256x256/apps/deb-installer.png"
install -m644 src-tauri/icons/icon.png "$ICONS/512x512/apps/deb-installer.png"
install -m644 packaging/deb-installer-mime.xml "$MIME/deb-installer.xml"

# WEBKIT_DISABLE_DMABUF_RENDERER wajib di sebagian driver; tanpa itu jendela
# terbuka tetapi isinya kosong. `env` dipakai karena Exec tidak bisa memuat
# penugasan variabel secara langsung.
cat > "$APPS/deb-installer.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=DebInstaller
GenericName=Debian Package Installer
Comment=Convert .deb packages into native pacman packages
Comment[id]=Konversi paket .deb menjadi paket pacman native
Exec=env WEBKIT_DISABLE_DMABUF_RENDERER=1 $BIN_DST %f
Icon=deb-installer
Terminal=false
Categories=System;PackageManager;
Keywords=deb;debian;install;package;convert;
MimeType=application/vnd.debian.binary-package;
StartupNotify=true
StartupWMClass=deb-installer
EOF

desktop-file-validate "$APPS/deb-installer.desktop" || true
update-desktop-database "$APPS" 2>/dev/null || true
update-mime-database "$HOME/.local/share/mime" 2>/dev/null || true
gtk-update-icon-cache -q -t -f "$ICONS" 2>/dev/null || true

echo "Terpasang:"
echo "  $BIN_DST"
echo "  $APPS/deb-installer.desktop"
echo "Cari \"DebInstaller\" di menu aplikasi."
