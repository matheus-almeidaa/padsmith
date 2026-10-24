{
  lib,
  rustPlatform,
  pkg-config,
  systemd,
  makeWrapper,
  libnotify,
}:
rustPlatform.buildRustPackage {
  pname = "padsmith-daemon";
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
    makeWrapper
  ];
  buildInputs = [ systemd ];
  cargoBuildFlags = [
    "-p"
    "padsmithd"
    "-p"
    "padsmith"
  ];
  cargoTestFlags = [
    "-p"
    "padsmith-core"
    "-p"
    "padsmith-linux"
    "-p"
    "padsmith-protocol"
    "-p"
    "padsmithd"
    "-p"
    "padsmith"
  ];
  postInstall = ''
    wrapProgram $out/bin/padsmithd --prefix PATH : ${
      lib.makeBinPath [
        libnotify
        systemd
      ]
    }
    wrapProgram $out/bin/padsmith --prefix PATH : ${lib.makeBinPath [ systemd ]}
  '';
  meta = {
    description = "Daemon e CLI de remapeamento de controles evdev/uinput";
    license = lib.licenses.mit;
    platforms = lib.platforms.linux;
    mainProgram = "padsmith";
  };
}
