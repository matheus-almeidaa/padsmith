{
  lib,
  rustPlatform,
  pkg-config,
  systemd,
  cargo-tauri,
  gtk3,
  webkitgtk_4_1,
  libayatana-appindicator,
  openssl,
  librsvg,
  wrapGAppsHook3,
  frontend,
}:
rustPlatform.buildRustPackage {
  pname = "padsmith-ui";
  version = "0.1.0";
  src = lib.cleanSourceWith {
    src = ../.;
    filter =
      path: type:
      !(builtins.elem (baseNameOf path) [
        ".git"
        "target"
        "node_modules"
        "dist"
        "result"
        ".playwright-mcp"
        "docs"
        "README.md"
      ])
      && !(lib.hasSuffix "/crates/ui/gen" path)
      && !(lib.hasSuffix "/crates/ui/icons/icon.png" path);
  };
  cargoLock.lockFile = ../Cargo.lock;
  nativeBuildInputs = [
    pkg-config
    cargo-tauri.hook
    wrapGAppsHook3
    librsvg
  ];
  buildInputs = [
    systemd
    gtk3
    webkitgtk_4_1
    libayatana-appindicator
    openssl
  ];
  buildAndTestSubdir = "crates/ui";
  cargoBuildFlags = [
    "--package"
    "padsmith-ui"
  ];
  cargoTestFlags = [
    "--package"
    "padsmith-ui"
  ];
  preBuild = ''
    cp -r ${frontend} ui/dist
    rsvg-convert -w 128 -h 128 ui/static/padsmith.svg -o crates/ui/icons/icon.png
  '';
  postInstall = ''
    install -Dm644 ui/static/padsmith.svg $out/share/icons/hicolor/scalable/apps/io.padsmith.desktop.svg
  '';
  preFixup = ''
    gappsWrapperArgs+=(--prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath [ libayatana-appindicator ]})
  '';
  meta = {
    description = "Interface gráfica Padsmith — controles do seu jeito";
    license = lib.licenses.mit;
    platforms = lib.platforms.linux;
    mainProgram = "padsmith-ui";
  };
}
