pub mod devices;
pub mod doctor;
pub mod output;
pub mod processes;
pub mod profiles;

use evdev::{AbsoluteAxisCode, KeyCode};
use std::{collections::BTreeMap, sync::LazyLock};

static KEYS: LazyLock<BTreeMap<String, u16>> = LazyLock::new(|| {
    (0..=0x2ff)
        .map(|i| (format!("{:?}", KeyCode(i)), i))
        .collect()
});
pub fn key_code(name: &str) -> Option<u16> {
    KEYS.get(name).copied().or_else(|| {
        name.strip_prefix("KEYCODE_")
            .and_then(|s| u16::from_str_radix(s, 16).ok())
            .filter(|c| *c <= 0x2ff)
    })
}
pub fn key_name(code: u16) -> String {
    let name = format!("{:?}", KeyCode(code));
    if name.starts_with("BTN_") || name.starts_with("KEY_") {
        name
    } else {
        format!("KEYCODE_{code:03x}")
    }
}
static AXES: LazyLock<BTreeMap<String, u16>> = LazyLock::new(|| {
    (0..=0x3f)
        .map(|i| (format!("{:?}", AbsoluteAxisCode(i)), i))
        .collect()
});
pub fn axis_code(name: &str) -> Option<u16> {
    AXES.get(name).copied().or_else(|| {
        name.strip_prefix("AXISCODE_")
            .and_then(|s| u16::from_str_radix(s, 16).ok())
            .filter(|c| *c <= 0x3f)
    })
}
pub fn axis_name(code: u16) -> String {
    let name = format!("{:?}", AbsoluteAxisCode(code));
    if name.starts_with("ABS_") {
        name
    } else {
        format!("AXISCODE_{code:02x}")
    }
}
pub fn validate_profile(profile: &padsmith_core::Profile) -> anyhow::Result<()> {
    profile.validate().map_err(anyhow::Error::msg)?;
    for source in profile.aliases.values() {
        anyhow::ensure!(
            key_code(source.rsplit(':').next().unwrap_or(source)).is_some(),
            "Código associado a traseiro inválido: {source}"
        );
    }
    for layer in profile.layers.values() {
        validate_profile(&padsmith_core::Profile {
            buttons: layer.buttons.clone(),
            axes: layer.axes.clone(),
            ..Default::default()
        })?;
        for source in &layer.activation {
            anyhow::ensure!(
                key_code(source.rsplit(':').next().unwrap_or(source)).is_some(),
                "Ativador de camada inválido: {source}"
            );
        }
    }
    for source in profile
        .emergency
        .buttons
        .iter()
        .chain(profile.shortcuts.iter().flat_map(|s| &s.buttons))
    {
        anyhow::ensure!(
            key_code(source.rsplit(':').next().unwrap_or(source)).is_some(),
            "Botão de atalho inválido: {source}"
        );
    }
    for source in profile.buttons.keys() {
        anyhow::ensure!(
            key_code(source.rsplit(':').next().unwrap_or(source)).is_some(),
            "Botão de entrada inválido: {source}"
        );
    }
    for trigger in &profile.triggers {
        for source in &trigger.buttons {
            anyhow::ensure!(
                key_code(source.rsplit(':').next().unwrap_or(source)).is_some(),
                "Botão de gatilho inválido: {source}"
            );
        }
    }
    for source in profile.axes.keys() {
        anyhow::ensure!(
            axis_code(source.rsplit(':').next().unwrap_or(source)).is_some(),
            "Eixo de entrada inválido: {source}"
        );
    }
    for target in profile
        .buttons
        .values()
        .filter_map(|b| b.output.as_ref())
        .chain(
            profile
                .macros
                .values()
                .flat_map(|m| padsmith_core::macros::targets(&m.steps)),
        )
        .chain(profile.axes.values().flat_map(|a| {
            [a.negative.as_ref(), a.positive.as_ref()]
                .into_iter()
                .flatten()
        }))
    {
        use padsmith_core::Target;
        match target {
            Target::Gamepad { code } => anyhow::ensure!(
                padsmith_core::GAMEPAD_BUTTONS.contains(&code.as_str()),
                "Botão virtual inválido: {code}"
            ),
            Target::Trigger { code } => anyhow::ensure!(
                ["ABS_Z", "ABS_RZ"].contains(&code.as_str()),
                "Gatilho virtual inválido: {code}"
            ),
            Target::Dpad { code, value } => anyhow::ensure!(
                ["ABS_HAT0X", "ABS_HAT0Y"].contains(&code.as_str()) && [-1, 1].contains(value),
                "Direção d-pad inválida"
            ),
            Target::Key { code } => anyhow::ensure!(
                code.starts_with("KEY_") && key_code(code).is_some(),
                "Tecla inválida: {code}"
            ),
            Target::Mouse { code } => anyhow::ensure!(
                [
                    "BTN_LEFT",
                    "BTN_RIGHT",
                    "BTN_MIDDLE",
                    "BTN_SIDE",
                    "BTN_EXTRA"
                ]
                .contains(&code.as_str()),
                "Botão de mouse inválido: {code}"
            ),
            Target::Disabled => {}
        }
    }
    Ok(())
}
