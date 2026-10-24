#!/usr/bin/env bash
set -euo pipefail

if (( $# > 0 )); then
  if (( $# == 1 )) && [[ "$1" == --help || "$1" == -h ]]; then
    printf 'Uso: clean-project\n\nRemove caches e artefatos de build do Padsmith.\nPreserva os AppImages e arquivos de distribuição em target/appimage/.\n'
    exit 0
  fi
  printf 'Uso: clean-project (sem argumentos).\n' >&2
  exit 2
fi

root=$(realpath "$(dirname "${BASH_SOURCE[0]}")/..")
if [[ ! -f "$root/Cargo.toml" || ! -f "$root/flake.nix" ]]; then
  printf 'Não foi possível identificar a raiz do projeto Padsmith.\n' >&2
  exit 1
fi

# Remove only known, regenerable paths. Never remove target/appimage itself.
rm -rf -- \
  "$root/target/debug" \
  "$root/target/release" \
  "$root/target/appimage/.cache" \
  "$root/target/appimage-sidecars" \
  "$root/ui/node_modules" \
  "$root/ui/dist" \
  "$root/.playwright-mcp"
rm -f -- "$root/target/.rustc_info.json" "$root/target/CACHEDIR.TAG"

printf 'Limpeza concluída. AppImages e arquivos de distribuição em target/appimage/ preservados.\n'
du -sh "$root"
