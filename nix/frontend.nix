{
  lib,
  stdenv,
  fetchPnpmDeps,
  nodejs,
  pnpm,
  pnpmConfigHook,
}:
stdenv.mkDerivation (finalAttrs: {
  pname = "padsmith-frontend";
  version = "0.1.0";
  src = lib.cleanSourceWith {
    src = ../ui;
    filter =
      path: type:
      !(builtins.elem (baseNameOf path) [
        "node_modules"
        "dist"
      ]);
  };
  pnpmDeps = fetchPnpmDeps {
    inherit (finalAttrs) pname version src;
    inherit pnpm;
    fetcherVersion = 4;
    hash = "sha256-zU3y2FedBGZSqPXMboADfpRTvWrNxYic7JFmOf71JN0=";
  };
  nativeBuildInputs = [
    nodejs
    pnpm
    pnpmConfigHook
  ];
  buildPhase = ''
    runHook preBuild
    pnpm build
    runHook postBuild
  '';
  installPhase = ''
    runHook preInstall
    cp -r dist $out
    runHook postInstall
  '';
})
