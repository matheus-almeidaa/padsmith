#!/usr/bin/env bash
set -euo pipefail

# Work on a private copy: never overwrite Nix-built binaries or host node_modules.
build=$(mktemp -d)
trap 'rm -rf "$build"' EXIT
tar -C /source --exclude='./.git' --exclude='./target' \
  --exclude='./ui/node_modules' --exclude='./ui/dist' --exclude='./.playwright-mcp' \
  -cf - . | tar -C "$build" -xf -
cd "$build"
export HOME=/out/.cache/home CARGO_HOME=/out/.cache/cargo CI=true
export CARGO_TARGET_DIR=/out/.cache/target
export TAURI_APP_PATH="$build/crates/ui" TAURI_FRONTEND_PATH="$build/ui"
mkdir -p "$HOME" target/appimage-sidecars

pnpm --dir ui install --frozen-lockfile
pnpm --dir ui build
rsvg-convert -w 128 -h 128 ui/static/padsmith.svg -o crates/ui/icons/icon.png
cargo build --release --locked -p padsmith -p padsmithd
target=$(rustc -vV | while IFS=' ' read -r key value; do
  if [[ "$key" == host: ]]; then printf '%s' "$value"; fi
done)
for binary in padsmith padsmithd; do
  install -m755 "$CARGO_TARGET_DIR/release/$binary" "target/appimage-sidecars/$binary-$target"
done

# NixOS may register an AppImage binfmt handler outside the container. Clear the
# optional AI marker on this build tool so Linux executes its ELF runtime directly.
tools="$HOME/.cache/tauri"
mkdir -p "$tools"
plugin="$tools/linuxdeploy-plugin-appimage.AppImage"
if [[ ! -f "$plugin" ]]; then
  wget -q "https://github.com/linuxdeploy/linuxdeploy-plugin-appimage/releases/download/continuous/linuxdeploy-plugin-appimage-$(uname -m).AppImage" -O "$plugin"
fi
chmod +x "$plugin"
printf '\0\0\0' | dd of="$plugin" bs=1 seek=8 count=3 conv=notrunc status=none
tauri build --verbose --ci --bundles appimage --config crates/ui/tauri.appimage.conf.json

shopt -s nullglob
images=("$CARGO_TARGET_DIR"/release/bundle/appimage/*.AppImage)
if [[ ${#images[@]} -ne 1 ]]; then
  printf 'Esperado um AppImage, encontrados %s.\n' "${#images[@]}" >&2
  exit 1
fi
install -m755 "${images[0]}" /out/
cp packaging/appimage/70-padsmith-uinput.rules packaging/appimage/71-padsmith-input.rules /out/
cp docs/appimage.md /out/README.md
cp docs/nixos.md /out/nixos.md
(cd /out && sha256sum ./*.AppImage > SHA256SUMS)
printf '\nAppImage gerado: %s\n' "${images[0]##*/}"
