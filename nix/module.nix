self:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.padsmith;
  selector = lib.types.submodule {
    options = {
      vendor = lib.mkOption {
        type = lib.types.strMatching "[0-9a-fA-F]{4}";
        description = "VID hexadecimal do controle";
      };
      product = lib.mkOption {
        type = lib.types.strMatching "[0-9a-fA-F]{4}";
        description = "PID hexadecimal do controle";
      };
      pathPattern = lib.mkOption {
        type = lib.types.nullOr (lib.types.strMatching "[a-zA-Z0-9_/:.*?+\\[\\]-]+");
        default = null;
        description = "Restrição opcional pelo DEVPATH (sem /sys), com glob udev";
      };
    };
  };
  match =
    device:
    ''KERNELS=="000[356]:${lib.toUpper device.vendor}:${lib.toUpper device.product}.*"''
    + lib.optionalString (device.pathPattern != null) '', DEVPATH=="${device.pathPattern}"'';
  inputRules = lib.concatMapStringsSep "\n" (
    d: ''SUBSYSTEM=="input", KERNEL=="event*", ${match d}, TAG+="uaccess", ENV{PADSMITH_MANAGED}="1"''
  ) cfg.devices;
  hiddenRules = lib.concatMapStringsSep "\n" (
    d:
    ''SUBSYSTEM=="hidraw", ${match d}, TAG-="uaccess", OWNER:="root", GROUP:="root", MODE:="0600", ENV{PADSMITH_HIDRAW_HIDDEN}="1"''
  ) cfg.devices;
  rules = pkgs.runCommand "padsmith-udev-rules" { } ''
    mkdir -p $out/lib/udev/rules.d
    cat > $out/lib/udev/rules.d/70-padsmith-uinput.rules <<'RULES'
    SUBSYSTEM=="misc", KERNEL=="uinput", OPTIONS+="static_node=uinput", TAG+="uaccess"
    RULES
    cat > $out/lib/udev/rules.d/71-padsmith-input.rules <<'RULES'
    ${inputRules}
    RULES
    ${lib.optionalString cfg.hideHidraw ''
      cat > $out/lib/udev/rules.d/72-padsmith-hidraw.rules <<'RULES'
      ${hiddenRules}
      RULES
    ''}
  '';
in
{
  options.services.padsmith = {
    enable = lib.mkEnableOption "Padsmith, daemon de remapeamento de controles por usuário";
    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      description = "Pacote Padsmith";
    };
    devices = lib.mkOption {
      type = lib.types.listOf selector;
      default = [ ];
      example = [
        {
          vendor = "054c";
          product = "09cc";
        }
        {
          vendor = "057e";
          product = "2009";
        }
      ];
      description = "Controles cujos nós irmãos recebem uaccess e cujo hidraw pode ser ocultado. Não restringe a descoberta do daemon.";
    };
    hideHidraw = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Restringe hidraw dos seletores devices, antes de 73-seat-late.rules. Persiste com o daemon parado.";
    };
    logLevel = lib.mkOption {
      type = lib.types.enum [
        "error"
        "warn"
        "info"
        "debug"
        "trace"
      ];
      default = "info";
      description = "Nível de tracing do daemon";
    };
    tray.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Inicia o ícone da bandeja na sessão gráfica";
    };
    rulesPackage = lib.mkOption {
      type = lib.types.package;
      readOnly = true;
      internal = true;
      default = rules;
    };
  };
  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = !cfg.hideHidraw || cfg.devices != [ ];
        message = "services.padsmith.hideHidraw exige seletores explícitos em services.padsmith.devices";
      }
    ];
    environment.systemPackages = [ cfg.package ];
    boot.kernelModules = [ "uinput" ];
    services.udev.packages = [ rules ];
    systemd.user.services.padsmith = {
      description = "Padsmith — remapeamento de controles";
      wantedBy = [ "graphical-session.target" ];
      partOf = [ "graphical-session.target" ];
      after = [ "graphical-session.target" ];
      serviceConfig = {
        Type = "notify";
        NotifyAccess = "main";
        ExecStart = "${cfg.package}/bin/padsmithd";
        Environment = "RUST_LOG=padsmithd=${cfg.logLevel},padsmith_linux=${cfg.logLevel}";
        Restart = "on-failure";
        RestartSec = "1s";
        WatchdogSec = "5s";
        WatchdogSignal = "SIGKILL";
        TimeoutStopSec = "3s";
        UMask = "0077";
        NoNewPrivileges = true;
      };
    };
    systemd.user.services.padsmith-tray = lib.mkIf cfg.tray.enable {
      description = "Padsmith — bandeja e interface";
      wantedBy = [ "graphical-session.target" ];
      partOf = [ "graphical-session.target" ];
      after = [
        "graphical-session.target"
        "padsmith.service"
      ];
      serviceConfig = {
        ExecStart = "${cfg.package}/bin/padsmith-ui --tray";
        Restart = "on-failure";
        RestartSec = "3s";
      };
    };
  };
}
