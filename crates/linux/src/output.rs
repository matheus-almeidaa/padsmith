use anyhow::{Context, Result};
use evdev::{
    AbsInfo, AbsoluteAxisCode, AttributeSet, BusType, InputEvent, InputId, KeyCode,
    RelativeAxisCode, UinputAbsSetup, uinput::VirtualDevice,
};
use padsmith_core::{GAMEPAD_AXES, GAMEPAD_BUTTONS, Output, Target};
use std::collections::{BTreeMap, BTreeSet};

pub fn controller_name(id: &str) -> String {
    let hash = id.bytes().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
    });
    format!("Padsmith Xbox 360 {hash:016x}")
}
// Profiles/core use physical positions. The Xbox 360 identity must instead
// expose xpad's historical ABI: X = BTN_X (alias BTN_NORTH, 0x133),
// Y = BTN_Y (alias BTN_WEST, 0x134). Keep this conversion at the output boundary;
// swapping the generic evdev names would also corrupt PlayStation inputs.
fn gamepad_key_code(code: &str) -> Option<u16> {
    match code {
        "BTN_WEST" => Some(KeyCode::BTN_NORTH.0),
        "BTN_NORTH" => Some(KeyCode::BTN_WEST.0),
        _ => crate::key_code(code),
    }
}
pub fn gamepad(name: &str) -> Result<VirtualDevice> {
    let keys: AttributeSet<KeyCode> = GAMEPAD_BUTTONS
        .iter()
        .map(|k| KeyCode(gamepad_key_code(k).expect("standard code")))
        .collect();
    let mut builder = VirtualDevice::builder()
        .context("Abrir uinput")?
        .name(name)
        .input_id(InputId::new(BusType::BUS_USB, 0x045e, 0x028e, 0x0114))
        .with_keys(&keys)
        .context("Capacidades de botões uinput")?;
    for code in GAMEPAD_AXES {
        let (min, max) = bounds(code);
        builder = builder
            .with_absolute_axis(&UinputAbsSetup::new(
                AbsoluteAxisCode(crate::axis_code(code).unwrap()),
                AbsInfo::new(0, min, max, 0, 0, 0),
            ))
            .with_context(|| format!("Capacidade uinput {code}"))?;
    }
    builder.build().context("UI_DEV_CREATE")
}
pub fn bounds(code: &str) -> (i32, i32) {
    if ["ABS_Z", "ABS_RZ"].contains(&code) {
        (0, 255)
    } else if code.starts_with("ABS_HAT") {
        (-1, 1)
    } else {
        (-32768, 32767)
    }
}
pub fn emit_gamepad(device: &mut VirtualDevice, outputs: &[Output]) -> Result<()> {
    let events = gamepad_events(outputs)?;
    if !events.is_empty() {
        device.emit(&events)?;
    }
    Ok(())
}
fn gamepad_events(outputs: &[Output]) -> Result<Vec<InputEvent>> {
    let mut events = Vec::new();
    for output in outputs {
        match output {
            Output::Button {
                target: Target::Gamepad { code },
                pressed,
            } => events.push(InputEvent::new(
                1,
                gamepad_key_code(code).context("Código inválido")?,
                i32::from(*pressed),
            )),
            Output::Axis { code, value } => {
                let (min, max) = bounds(code);
                let value = if min == 0 {
                    value.clamp(0.0, 1.0) * max as f64
                } else if *value < 0.0 {
                    value.clamp(-1.0, 0.0) * -(min as f64)
                } else {
                    value.clamp(0.0, 1.0) * max as f64
                };
                events.push(InputEvent::new(
                    3,
                    crate::axis_code(code).context("Eixo inválido")?,
                    value.round() as i32,
                ));
            }
            _ => {}
        }
    }
    Ok(events)
}

/// Shared desktop devices track ownership across controllers, not only within one engine.
pub struct Desktop {
    pub keyboard: VirtualDevice,
    pub mouse: VirtualDevice,
    owners: BTreeMap<Target, BTreeSet<String>>,
}
impl Desktop {
    pub fn new() -> Result<Self> {
        let keys: AttributeSet<KeyCode> = (1..=0x2ff)
            .filter(|k| format!("{:?}", KeyCode(*k)).starts_with("KEY_"))
            .map(KeyCode)
            .collect();
        let mouse: AttributeSet<KeyCode> = [
            KeyCode::BTN_LEFT,
            KeyCode::BTN_RIGHT,
            KeyCode::BTN_MIDDLE,
            KeyCode::BTN_SIDE,
            KeyCode::BTN_EXTRA,
        ]
        .into_iter()
        .collect();
        let rel: AttributeSet<RelativeAxisCode> = [
            RelativeAxisCode::REL_X,
            RelativeAxisCode::REL_Y,
            RelativeAxisCode::REL_WHEEL,
        ]
        .into_iter()
        .collect();
        Ok(Self {
            keyboard: VirtualDevice::builder()?
                .name("Padsmith Keyboard")
                .with_keys(&keys)?
                .build()?,
            mouse: VirtualDevice::builder()?
                .name("Padsmith Mouse")
                .with_keys(&mouse)?
                .with_relative_axes(&rel)?
                .build()?,
            owners: BTreeMap::new(),
        })
    }
    pub fn emit(&mut self, owner: &str, outputs: &[Output]) -> Result<()> {
        let mut keys = Vec::new();
        let mut mouse = Vec::new();
        for output in outputs {
            match output {
                Output::Button { target, pressed }
                    if matches!(target, Target::Key { .. } | Target::Mouse { .. }) =>
                {
                    let owners = self.owners.entry(target.clone()).or_default();
                    let before = !owners.is_empty();
                    if *pressed {
                        owners.insert(owner.into());
                    } else {
                        owners.remove(owner);
                    }
                    if before == !owners.is_empty() {
                        continue;
                    }
                    let (code, destination) = match target {
                        Target::Key { code } => (code, &mut keys),
                        Target::Mouse { code } => (code, &mut mouse),
                        _ => unreachable!(),
                    };
                    destination.push(InputEvent::new(
                        1,
                        crate::key_code(code).context("Código de saída inválido")?,
                        i32::from(*pressed),
                    ));
                }
                Output::Relative { code, value } => {
                    let code = if code == "REL_X" {
                        0
                    } else if code == "REL_Y" {
                        1
                    } else {
                        continue;
                    };
                    mouse.push(InputEvent::new(2, code, *value));
                }
                _ => {}
            }
        }
        self.owners.retain(|_, v| !v.is_empty());
        if !keys.is_empty() {
            self.keyboard.emit(&keys)?;
        }
        if !mouse.is_empty() {
            self.mouse.emit(&mouse)?;
        }
        Ok(())
    }
    pub fn release(&mut self, owner: &str) -> Result<()> {
        let outputs: Vec<_> = self
            .owners
            .iter()
            .filter(|(_, v)| v.contains(owner))
            .map(|(t, _)| Output::Button {
                target: t.clone(),
                pressed: false,
            })
            .collect();
        self.emit(owner, &outputs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use padsmith_core::{Profile, State, step};
    use padsmith_protocol::NodeInfo;

    #[test]
    fn positional_face_buttons_match_xpad_wire_codes() {
        let node = NodeInfo {
            role: "gamepad".into(),
            primary: true,
            ..Default::default()
        };
        // hid-playstation positions -> xpad's A/B/X/Y ABI (BTN_X=0x133,
        // BTN_Y=0x134). Labels must not change the emitted Xbox button.
        for layout in ["xbox", "playstation", "nintendo"] {
            let profile = Profile {
                layout: layout.into(),
                ..Default::default()
            };
            for (physical, xbox) in [
                (0x130, 0x130),
                (0x131, 0x131),
                (0x134, 0x133),
                (0x133, 0x134),
            ] {
                let mut state = State::default();
                for value in [1, 0] {
                    let input = crate::devices::normalize(
                        &node,
                        &BTreeMap::new(),
                        InputEvent::new(1, physical, value),
                    )
                    .unwrap();
                    let result = step(state, &profile, input, (1 - value) as u64);
                    let events = gamepad_events(&result.output).unwrap();
                    assert_eq!(events.len(), 1);
                    assert_eq!(
                        (events[0].code(), events[0].value()),
                        (xbox, value),
                        "layout={layout}, physical={physical:#x}"
                    );
                    state = result.state;
                }
            }
        }
    }

    #[test]
    fn explicit_xy_swap_emits_only_the_requested_xbox_button() {
        let profile: Profile =
            toml::from_str(include_str!("../../../examples/swap-xy.toml")).unwrap();
        for (source, xbox) in [("BTN_WEST", 0x134), ("BTN_NORTH", 0x133)] {
            let mut state = State::default();
            for pressed in [true, false] {
                let result = step(
                    state,
                    &profile,
                    padsmith_core::Input::Button {
                        code: source.into(),
                        pressed,
                    },
                    u64::from(!pressed),
                );
                let events = gamepad_events(&result.output).unwrap();
                assert_eq!(events.len(), 1);
                assert_eq!(
                    (events[0].code(), events[0].value()),
                    (xbox, i32::from(pressed))
                );
                state = result.state;
            }
        }
    }
}
