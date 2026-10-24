#!/usr/bin/env bash
set -euo pipefail
root=$(realpath "$(dirname "${BASH_SOURCE[0]}")/..")
engine=${CONTAINER_ENGINE:-}
if [[ -z "$engine" ]]; then
  if command -v podman >/dev/null 2>&1; then engine=podman
  elif command -v docker >/dev/null 2>&1; then engine=docker
  else
    printf 'Instale Podman ou Docker. No NixOS: nix shell --inputs-from . nixpkgs#podman --command bash scripts/build-appimage.sh\n' >&2
    exit 1
  fi
fi
image=localhost/padsmith-appimage-builder
userns=()
build_options=()
if [[ "${engine##*/}" == podman ]]; then
  userns=(--userns=keep-id)
  # A temporary `nix shell ...#podman` does not install /etc/containers/policy.json.
  if [[ ! -f "${XDG_CONFIG_HOME:-$HOME/.config}/containers/policy.json" && ! -f /etc/containers/policy.json ]]; then
    build_options=(--signature-policy "$root/packaging/appimage/policy.json")
  fi
fi
mkdir -p "$root/target/appimage"
"$engine" build "${build_options[@]}" -t "$image" -f "$root/packaging/appimage/Dockerfile" "$root/packaging/appimage"
"$engine" run --rm --pull=never "${userns[@]}" --user "$(id -u):$(id -g)" \
  --env "CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-4}" \
  --mount "type=bind,src=$root,dst=/source,readonly" \
  --mount "type=bind,src=$root/target/appimage,dst=/out" \
  "$image"
