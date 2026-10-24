use anyhow::Result;
use padsmith_protocol::Status;
use serde::Serialize;
use std::fs::OpenOptions;

#[derive(Debug, Serialize)]
pub struct Check {
    pub subject: String,
    pub level: String,
    pub detail: String,
}
fn check(
    checks: &mut Vec<Check>,
    subject: impl Into<String>,
    level: &str,
    detail: impl Into<String>,
) {
    checks.push(Check {
        subject: subject.into(),
        level: level.into(),
        detail: detail.into(),
    });
}
pub fn run(status: Option<&Status>, probe: bool) -> Result<Vec<Check>> {
    let mut checks = Vec::new();
    for name in [
        "70-padsmith-uinput.rules",
        "71-padsmith-input.rules",
        "72-padsmith-hidraw.rules",
    ] {
        let installed = [
            "/etc/udev/rules.d",
            "/usr/lib/udev/rules.d",
            "/lib/udev/rules.d",
        ]
        .iter()
        .any(|p| std::path::Path::new(p).join(name).is_file());
        check(
            &mut checks,
            format!("udev / {name}"),
            if installed { "ok" } else { "info" },
            if installed {
                "Arquivo de regra instalado; propriedades e permissões efetivas são verificadas abaixo"
            } else {
                "Regra Padsmith não encontrada nos diretórios udev padrão; outras regras podem conceder acesso"
            },
        );
    }
    if status.is_none() {
        check(
            &mut checks,
            "Daemon / IPC",
            "info",
            "Daemon indisponível ou com protocolo incompatível; diagnóstico de dispositivos funciona sem ele",
        );
    }
    match OpenOptions::new().write(true).open("/dev/uinput") {
        Ok(_) => check(
            &mut checks,
            "/dev/uinput",
            "ok",
            "Escrita permitida; nenhum dispositivo foi criado",
        ),
        Err(e) => check(&mut checks, "/dev/uinput", "error", e.to_string()),
    }
    match crate::devices::kwin_gamecontroller() {
        Some(false) => check(
            &mut checks,
            "KWin / kwinrc",
            "ok",
            "Game Controller desativado na configuração gravada",
        ),
        Some(true) => check(
            &mut checks,
            "KWin / kwinrc",
            "warning",
            "Game Controller ativado: pode converter inclusive o controle virtual em mouse/teclado",
        ),
        None => check(
            &mut checks,
            "KWin / kwinrc",
            "warning",
            "Opção não declarada; estado efetivo não confirmado",
        ),
    }
    // A running plugin is observable via KWin's plugin management D-Bus API when available.
    match std::process::Command::new("busctl")
        .args([
            "--user",
            "--full",
            "--timeout=2s",
            "get-property",
            "org.kde.KWin",
            "/Plugins",
            "org.kde.KWin.Plugins",
            "LoadedPlugins",
        ])
        .output()
    {
        Ok(output) if output.status.success() => {
            let loaded = String::from_utf8_lossy(&output.stdout)
                .split_ascii_whitespace()
                .any(|s| {
                    matches!(
                        s.trim_matches('"').to_ascii_lowercase().as_str(),
                        "gamecontroller" | "gamecontrollerplugin"
                    )
                });
            check(
                &mut checks,
                "KWin / sessão",
                if loaded { "warning" } else { "ok" },
                if loaded {
                    "Plugin gamecontroller carregado"
                } else {
                    "Plugin gamecontroller não carregado"
                },
            );
        }
        _ => check(
            &mut checks,
            "KWin / sessão",
            "info",
            "API D-Bus indisponível; não foi possível confirmar o plugin em execução",
        ),
    }
    let devices = crate::devices::discover()?;
    if devices.is_empty() {
        check(
            &mut checks,
            "Controles",
            "info",
            "Nenhum gamepad evdev encontrado",
        );
    }
    for device in devices {
        let managed = status.is_some_and(|s| {
            s.controllers
                .iter()
                .any(|c| c.device.id == device.id && c.error.is_none())
        });
        check(
            &mut checks,
            format!("{} ({}:{})", device.name, device.vendor, device.product),
            "info",
            format!("driver={} id={}", device.driver, device.id),
        );
        for node in &device.nodes {
            match evdev::raw_stream::RawDevice::open(&node.path) {
                Ok(mut input) => {
                    if managed {
                        check(
                            &mut checks,
                            &node.path,
                            "ok",
                            "Leitura permitida; captura pelo próprio Padsmith",
                        );
                    } else if probe {
                        match input.grab() {
                            Ok(()) => {
                                input.ungrab()?;
                                check(
                                    &mut checks,
                                    &node.path,
                                    "ok",
                                    "Leitura permitida; teste breve EVIOCGRAB livre",
                                );
                            }
                            Err(e) if e.raw_os_error() == Some(libc::EBUSY) => check(
                                &mut checks,
                                &node.path,
                                "warning",
                                "EVIOCGRAB ocupado por outro processo; o kernel não informa o proprietário",
                            ),
                            Err(e) => check(
                                &mut checks,
                                &node.path,
                                "error",
                                format!("Falha no teste de captura: {e}"),
                            ),
                        }
                    } else {
                        check(
                            &mut checks,
                            &node.path,
                            "ok",
                            "Leitura permitida; captura exclusiva não sondada",
                        );
                    }
                }
                Err(e) => check(
                    &mut checks,
                    &node.path,
                    "error",
                    format!("Sem leitura: {e}. Verifique uaccess para todos os nós irmãos"),
                ),
            }
        }
        for path in &device.hidraw {
            match OpenOptions::new().read(true).open(path) {
                Ok(_) => check(
                    &mut checks,
                    path,
                    "warning",
                    "Leitura permitida: Steam/SDL podem acessar o controle físico por hidraw",
                ),
                Err(e)
                    if device.hidraw_hidden && e.kind() == std::io::ErrorKind::PermissionDenied =>
                {
                    check(
                        &mut checks,
                        path,
                        "ok",
                        "Acesso negado como esperado; PADSMITH_HIDRAW_HIDDEN=1",
                    )
                }
                Err(e) => check(
                    &mut checks,
                    path,
                    "info",
                    format!(
                        "Acesso negado ou indisponível: {e}; marcador de regra Padsmith={}",
                        device.hidraw_hidden
                    ),
                ),
            }
        }
        check(
            &mut checks,
            format!("udev / {}", device.id),
            if device.hidraw_hidden { "ok" } else { "info" },
            if device.hidraw_hidden {
                "Regra opcional de ocultação aplicada ao dispositivo"
            } else {
                "Ocultação Padsmith não detectada; é uma opção independente do grab"
            },
        );
    }
    Ok(checks)
}
