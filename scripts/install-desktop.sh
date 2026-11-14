#!/usr/bin/env bash
# Installs the app icon into the hicolor icon theme and a launcher entry, so
# the app shows its icon on Wayland (app_id lookup), in app menus, and in
# trays that resolve icons by theme name.
set -euo pipefail
cd "$(dirname "$0")/.."

data_home="${XDG_DATA_HOME:-$HOME/.local/share}"

for size in 32x32 128x128 256x256; do
  dir="$data_home/icons/hicolor/$size/apps"
  mkdir -p "$dir"
  cp "assets/icon-${size%x*}.png" "$dir/pkgport.png"
done
mkdir -p "$data_home/icons/hicolor/scalable/apps"
cp assets/icon.svg "$data_home/icons/hicolor/scalable/apps/pkgport.svg"

# A launcher entry needs an executable it can actually find: install the
# release binary to ~/.local/bin (falls back to the debug build).
bin_dir="${HOME}/.local/bin"
mkdir -p "$bin_dir"
if [[ -f target/release/pkgport ]]; then
  cp target/release/pkgport "$bin_dir/"
elif [[ -f target/debug/pkgport ]]; then
  cp target/debug/pkgport "$bin_dir/"
fi
if [[ -f "$bin_dir/pkgport" && -f "$bin_dir/ps-downlowder" ]]; then
  rm "$bin_dir/ps-downlowder"
fi

mkdir -p "$data_home/applications"
if [[ -f "$data_home/applications/ps-downlowder.desktop" ]]; then
  mv "$data_home/applications/ps-downlowder.desktop" "$data_home/applications/pkgport.desktop"
fi
cp assets/pkgport.desktop "$data_home/applications/"

update-desktop-database "$data_home/applications" 2>/dev/null || true
gtk-update-icon-cache -f "$data_home/icons/hicolor" 2>/dev/null || true
echo "Installed: launcher entry + icons in $data_home"
