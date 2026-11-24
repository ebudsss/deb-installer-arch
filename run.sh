#!/bin/sh
# Menjalankan DebInstaller dari hasil build lokal.
#
# WEBKIT_DISABLE_DMABUF_RENDERER=1 diperlukan pada sebagian driver GPU
# (mis. NVIDIA proprietary atau Mesa tanpa DRI3): tanpa itu WebKitGTK gagal
# membuat GBM buffer dan jendela tampil kosong.
set -e
cd "$(dirname "$0")"

BIN="target/debug/deb-installer"
if [ ! -x "$BIN" ]; then
  echo "Binari belum ada. Jalankan dulu:" >&2
  echo "  pnpm install && pnpm build" >&2
  echo "  cargo build -p deb-installer --features custom-protocol" >&2
  exit 1
fi

if [ ! -f dist/index.html ]; then
  echo "Aset frontend belum dibangun. Jalankan: pnpm build" >&2
  exit 1
fi

export WEBKIT_DISABLE_DMABUF_RENDERER="${WEBKIT_DISABLE_DMABUF_RENDERER:-1}"
exec "$BIN" "$@"
