//! Deterministic input transducer. Time is supplied by the caller in microseconds.
//! No clocks, threads, file descriptors or platform dependencies live here.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
mod controls;
pub mod macros;
pub use controls::{Emergency, Layer, ProfileShortcut};
pub use macros::{Macro, MacroStep, MacroTrigger, Recorder, Recording, TriggerKind};

pub const GAMEPAD_BUTTONS: &[&str] = &[
    "BTN_SOUTH",
    "BTN_EAST",
    "BTN_NORTH",
    "BTN_WEST",
    "BTN_TL",
    "BTN_TR",
    "BTN_SELECT",
    "BTN_START",
    "BTN_MODE",
    "BTN_THUMBL",
    "BTN_THUMBR",
];
pub const GAMEPAD_AXES: &[&str] = &[
    "ABS_X",
    "ABS_Y",
    "ABS_RX",
    "ABS_RY",
    "ABS_Z",
    "ABS_RZ",
    "ABS_HAT0X",
    "ABS_HAT0Y",
];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Target {
    Gamepad { code: String },
    Trigger { code: String },
    Dpad { code: String, value: i32 },
    Key { code: String },
    Mouse { code: String },
    Disabled,
}
pub fn default_target(code: &str) -> Option<Target> {
    match code {
        "BTN_TL2" => Some(Target::Trigger {
            code: "ABS_Z".into(),
        }),
        "BTN_TR2" => Some(Target::Trigger {
            code: "ABS_RZ".into(),
        }),
        "BTN_DPAD_LEFT" => Some(Target::Dpad {
            code: "ABS_HAT0X".into(),
            value: -1,
        }),
        "BTN_DPAD_RIGHT" => Some(Target::Dpad {
            code: "ABS_HAT0X".into(),
            value: 1,
        }),
        "BTN_DPAD_UP" => Some(Target::Dpad {
            code: "ABS_HAT0Y".into(),
            value: -1,
        }),
        "BTN_DPAD_DOWN" => Some(Target::Dpad {
            code: "ABS_HAT0Y".into(),
            value: 1,
        }),
        _ => GAMEPAD_BUTTONS
            .contains(&code)
            .then(|| Target::Gamepad { code: code.into() }),
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ButtonMode {
    #[default]
    Hold,
    Toggle,
    Turbo,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Binding {
    pub output: Option<Target>,
    pub mode: ButtonMode,
    pub hz: f64,
}
impl Default for Binding {
    fn default() -> Self {
        Self {
            output: None,
            mode: ButtonMode::Hold,
            hz: 10.0,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AxisMode {
    #[default]
    Gamepad,
    Keys,
    Mouse,
}

fn one() -> f64 {
    1.0
}
fn threshold() -> f64 {
    0.5
}
fn sensitivity() -> f64 {
    800.0
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct AxisBinding {
    pub mode: AxisMode,
    pub target: Option<String>,
    pub invert: bool,
    pub deadzone: f64,
    pub curve: f64,
    pub threshold: f64,
    pub negative: Option<Target>,
    pub positive: Option<Target>,
    /// Pixels per second at full deflection. Mouse targets are REL_X / REL_Y.
    pub sensitivity: f64,
}
impl Default for AxisBinding {
    fn default() -> Self {
        Self {
            mode: AxisMode::Gamepad,
            target: None,
            invert: false,
            deadzone: 0.0,
            curve: one(),
            threshold: threshold(),
            negative: None,
            positive: None,
            sensitivity: sensitivity(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Profile {
    pub version: u32,
    pub id: String,
    pub name: String,
    pub layout: String,
    pub buttons: BTreeMap<String, Binding>,
    pub axes: BTreeMap<String, AxisBinding>,
    pub macros: BTreeMap<String, Macro>,
    pub triggers: Vec<MacroTrigger>,
    pub processes: Vec<String>,
    pub devices: Vec<String>,
    pub priority: i32,
    pub layers: BTreeMap<String, Layer>,
    pub emergency: Emergency,
    pub shortcuts: Vec<ProfileShortcut>,
    pub aliases: BTreeMap<String, String>,
}
impl Default for Profile {
    fn default() -> Self {
        Self {
            version: 1,
            id: "default".into(),
            name: "Padrão".into(),
            layout: "xbox".into(),
            buttons: BTreeMap::new(),
            axes: BTreeMap::new(),
            macros: BTreeMap::new(),
            triggers: Vec::new(),
            processes: Vec::new(),
            devices: Vec::new(),
            priority: 0,
            layers: BTreeMap::new(),
            emergency: Emergency::default(),
            shortcuts: Vec::new(),
            aliases: BTreeMap::new(),
        }
    }
}
impl Profile {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("Versão de perfil não suportada".into());
        }
        if self.id.is_empty()
            || self.id.len() > 80
            || !self
                .id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
        {
            return Err(
                "ID deve conter apenas letras ASCII, números, _ e - (até 80 caracteres)".into(),
            );
        }
        if self.name.trim().is_empty() {
            return Err("O perfil precisa de um nome".into());
        }
        if !["xbox", "playstation", "nintendo"].contains(&self.layout.as_str()) {
            return Err("Layout inválido".into());
        }
        for binding in self.buttons.values() {
            if !binding.hz.is_finite() || !(1.0..=100.0).contains(&binding.hz) {
                return Err("Turbo deve ter frequência de 1 a 100 Hz".into());
            }
        }
        macros::validate(self)?;
        controls::validate(self)?;
        for (source, axis) in &self.axes {
            if !axis.deadzone.is_finite()
                || !(0.0..1.0).contains(&axis.deadzone)
                || !axis.curve.is_finite()
                || !(0.1..=10.0).contains(&axis.curve)
                || !axis.threshold.is_finite()
                || !(0.01..=1.0).contains(&axis.threshold)
                || !axis.sensitivity.is_finite()
                || !(1.0..=10000.0).contains(&axis.sensitivity)
            {
                return Err(format!("Parâmetros inválidos no eixo {source}"));
            }
            if axis.mode == AxisMode::Gamepad
                && !GAMEPAD_AXES.contains(&axis.target.as_deref().unwrap_or(source))
            {
                return Err(format!("Eixo de saída inválido: {source}"));
            }
            if axis.mode == AxisMode::Mouse
                && !["REL_X", "REL_Y"].contains(&axis.target.as_deref().unwrap_or(""))
            {
                return Err("Eixo de mouse deve ter target REL_X ou REL_Y".into());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Input {
    /// Auxiliary nodes use a qualified code, for example touchpad:BTN_LEFT.
    Button {
        code: String,
        pressed: bool,
    },
    /// Signed axes normalized to [-1,1], triggers to [0,1].
    Axis {
        code: String,
        value: f64,
    },
    Tick,
    Reset,
    Macro {
        macro_id: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Output {
    Button { target: Target, pressed: bool },
    Axis { code: String, value: f64 },
    Relative { code: String, value: i32 },
}

#[derive(Clone, Debug, Default)]
pub struct State {
    pub paused: bool,
    down: BTreeSet<String>,
    owners: BTreeMap<String, BTreeSet<Target>>,
    axes: BTreeMap<String, f64>,
    mouse: BTreeMap<String, (String, f64)>,
    remainder: BTreeMap<String, f64>,
    mouse_tick: Option<u64>,
    runs: BTreeMap<String, macros::Run>,
    gestures: BTreeMap<String, macros::Gesture>,
    turbo: BTreeMap<String, macros::Turbo>,
    axis_owners: BTreeMap<String, BTreeMap<String, (u64, f64)>>,
    axis_sequence: u64,
    active_layers: Vec<String>,
    raw_axes: BTreeMap<String, f64>,
    emergency_gesture: controls::Gesture,
    shortcut_gestures: BTreeMap<usize, controls::Gesture>,
}
impl State {
    pub fn active_layers(&self) -> &[String] {
        &self.active_layers
    }
    fn buttons(&self) -> BTreeSet<Target> {
        self.owners
            .values()
            .flat_map(|v| v.iter().cloned())
            .collect()
    }
    fn own(&mut self, owner: &str, target: Option<Target>) {
        self.owners.remove(owner);
        if let Some(t) = target.filter(|t| *t != Target::Disabled) {
            self.owners.insert(owner.into(), BTreeSet::from([t]));
        }
    }
    fn axis(&mut self, owner: &str, code: &str, value: Option<f64>, output: &mut Vec<Output>) {
        let sources = self.axis_owners.entry(code.into()).or_default();
        if let Some(value) = value {
            self.axis_sequence += 1;
            sources.insert(owner.into(), (self.axis_sequence, value));
        } else {
            sources.remove(owner);
        }
        let value = if ["ABS_Z", "ABS_RZ"].contains(&code) {
            sources
                .values()
                .map(|(_, v)| v.clamp(0.0, 1.0))
                .fold(0.0, f64::max)
        } else {
            sources
                .values()
                .max_by_key(|(sequence, _)| sequence)
                .map(|(_, v)| *v)
                .unwrap_or(0.0)
        };
        if self.axes.get(code).copied().unwrap_or(0.0) != value {
            self.axes.insert(code.into(), value);
            output.push(Output::Axis {
                code: code.into(),
                value,
            });
        }
    }
    fn release_axis_owner(&mut self, owner: &str, output: &mut Vec<Output>) {
        let codes: Vec<_> = self
            .axis_owners
            .iter()
            .filter(|(_, sources)| sources.contains_key(owner))
            .map(|(code, _)| code.clone())
            .collect();
        for code in codes {
            self.axis(owner, &code, None, output);
        }
    }
}
fn flush_buttons(before: &mut BTreeSet<Target>, state: &mut State, output: &mut Vec<Output>) {
    let after = state.buttons();
    for target in before.difference(&after) {
        if let Target::Trigger { code } = target {
            state.axis(&format!("trigger:{code}"), code, None, output);
            continue;
        }
        if let Target::Dpad { code, value } = target {
            state.axis(&format!("dpad:{code}:{value}"), code, None, output);
            continue;
        }
        output.push(Output::Button {
            target: target.clone(),
            pressed: false,
        });
    }
    for target in after.difference(before) {
        if let Target::Trigger { code } = target {
            state.axis(&format!("trigger:{code}"), code, Some(1.0), output);
            continue;
        }
        if let Target::Dpad { code, value } = target {
            state.axis(
                &format!("dpad:{code}:{value}"),
                code,
                Some(*value as f64),
                output,
            );
            continue;
        }
        output.push(Output::Button {
            target: target.clone(),
            pressed: true,
        });
    }
    *before = after;
}

#[derive(Debug)]
pub struct Transition {
    pub state: State,
    pub output: Vec<Output>,
    pub next_deadline: Option<u64>,
    pub effects: Vec<Effect>,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    PauseChanged { paused: bool },
    SelectProfile { profile: String },
    LayersChanged,
}

/// Pure transition: callers own state and pass monotonic time. No hidden time source.
pub fn step(mut state: State, profile: &Profile, input: Input, now: u64) -> Transition {
    let mut before = state.buttons();
    let mut output = Vec::new();
    let mut effects = Vec::new();
    match input {
        Input::Macro { macro_id } if !state.paused => {
            macros::play(&mut state, profile, &macro_id, now, &mut output)
        }
        Input::Reset => {
            if !state.active_layers.is_empty() {
                effects.push(Effect::LayersChanged);
            }
            for code in state.axes.keys() {
                output.push(Output::Axis {
                    code: code.clone(),
                    value: 0.0,
                });
            }
            state = State {
                paused: state.paused,
                ..State::default()
            };
        }
        Input::Button { code, pressed } => {
            let changed = if pressed {
                state.down.insert(code.clone())
            } else {
                state.down.remove(&code)
            };
            if changed {
                controls::layers(&mut state, profile, now, &mut output, &mut effects);
                controls::button(&mut state, profile, &code, pressed, true, now);
            }
        }
        Input::Axis { code, value } if value.is_finite() => {
            controls::layers(&mut state, profile, now, &mut output, &mut effects);
            state.raw_axes.insert(code.clone(), value);
            controls::axis(&mut state, profile, &code, value, now, &mut output);
        }
        Input::Tick => {
            controls::layers(&mut state, profile, now, &mut output, &mut effects);
            if let Some(last) = state.mouse_tick {
                let seconds = now.saturating_sub(last).min(50_000) as f64 / 1_000_000.0;
                let mut velocities = BTreeMap::<String, f64>::new();
                for (code, velocity) in state.mouse.values() {
                    *velocities.entry(code.clone()).or_default() += velocity;
                }
                for (code, velocity) in velocities {
                    let remainder = state.remainder.entry(code.clone()).or_default();
                    let pixels = velocity * seconds + *remainder;
                    let value = pixels.trunc() as i32;
                    *remainder = pixels - value as f64;
                    if value != 0 {
                        output.push(Output::Relative { code, value });
                    }
                }
                state.mouse_tick = if state.mouse.is_empty() {
                    None
                } else {
                    Some(now)
                };
            }
        }
        _ => {}
    }
    controls::advance(&mut state, profile, now, &mut output, &mut effects);
    flush_buttons(&mut before, &mut state, &mut output);
    if !state.paused {
        macros::gestures(&mut state, profile, now, &mut before, &mut output);
        macros::advance(&mut state, now, &mut before, &mut output);
    }
    flush_buttons(&mut before, &mut state, &mut output);
    let mouse_deadline = if state.mouse.is_empty() {
        None
    } else {
        Some(state.mouse_tick.unwrap_or(now) + 8_000)
    };
    let next_deadline = mouse_deadline
        .into_iter()
        .chain(macros::deadline(&state, profile, now))
        .chain(controls::deadline(&state, profile))
        .min();
    Transition {
        state,
        output,
        next_deadline,
        effects,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn press(code: &str, pressed: bool) -> Input {
        Input::Button {
            code: code.into(),
            pressed,
        }
    }
    fn pad(code: &str) -> Target {
        Target::Gamepad { code: code.into() }
    }
    #[test]
    fn swap_discards_original_and_releases_destination() {
        let mut p = Profile::default();
        for (a, b) in [("BTN_WEST", "BTN_NORTH"), ("BTN_NORTH", "BTN_WEST")] {
            p.buttons.insert(
                a.into(),
                Binding {
                    output: Some(pad(b)),
                    ..Default::default()
                },
            );
        }
        let a = step(State::default(), &p, press("BTN_WEST", true), 0);
        assert_eq!(
            a.output,
            vec![Output::Button {
                target: pad("BTN_NORTH"),
                pressed: true
            }]
        );
        let b = step(a.state, &p, press("BTN_WEST", false), 1);
        assert_eq!(
            b.output,
            vec![Output::Button {
                target: pad("BTN_NORTH"),
                pressed: false
            }]
        );
        assert_eq!(
            step(b.state, &p, press("BTN_NORTH", true), 2).output,
            vec![Output::Button {
                target: pad("BTN_WEST"),
                pressed: true
            }]
        );
    }
    #[test]
    fn toggle_ignores_release_and_repeat_and_reset_releases() {
        let mut p = Profile::default();
        p.buttons.insert(
            "BTN_SOUTH".into(),
            Binding {
                output: Some(pad("BTN_SOUTH")),
                mode: ButtonMode::Toggle,
                ..Default::default()
            },
        );
        let a = step(State::default(), &p, press("BTN_SOUTH", true), 0);
        assert_eq!(a.output.len(), 1);
        let a = step(a.state, &p, press("BTN_SOUTH", true), 1);
        assert!(a.output.is_empty());
        let a = step(a.state, &p, press("BTN_SOUTH", false), 2);
        assert!(a.output.is_empty());
        let a = step(a.state, &p, press("BTN_SOUTH", true), 3);
        assert_eq!(
            a.output,
            vec![Output::Button {
                target: pad("BTN_SOUTH"),
                pressed: false
            }]
        );
        let a = step(a.state, &p, press("BTN_SOUTH", false), 4);
        let a = step(a.state, &p, press("BTN_SOUTH", true), 5);
        assert_eq!(
            step(a.state, &p, Input::Reset, 6).output,
            vec![Output::Button {
                target: pad("BTN_SOUTH"),
                pressed: false
            }]
        );
    }
    #[test]
    fn overlapping_sources_do_not_release_each_other() {
        let mut p = Profile::default();
        p.buttons.insert(
            "BTN_WEST".into(),
            Binding {
                output: Some(pad("BTN_SOUTH")),
                ..Default::default()
            },
        );
        let a = step(State::default(), &p, press("BTN_SOUTH", true), 0);
        let a = step(a.state, &p, press("BTN_WEST", true), 1);
        assert!(a.output.is_empty());
        let a = step(a.state, &p, press("BTN_SOUTH", false), 2);
        assert!(a.output.is_empty());
        assert_eq!(
            step(a.state, &p, press("BTN_WEST", false), 3).output.len(),
            1
        );
    }
    #[test]
    fn axes_deadzone_curve_and_digital_trigger() {
        let mut p = Profile::default();
        p.axes.insert(
            "ABS_X".into(),
            AxisBinding {
                deadzone: 0.2,
                invert: true,
                curve: 2.0,
                target: Some("ABS_RX".into()),
                ..Default::default()
            },
        );
        let a = step(
            State::default(),
            &p,
            Input::Axis {
                code: "ABS_X".into(),
                value: 0.6,
            },
            0,
        );
        assert!(
            matches!(&a.output[0],Output::Axis {code,value} if code == "ABS_RX" && (value+0.25).abs()<1e-9)
        );
        p.axes.insert(
            "ABS_Z".into(),
            AxisBinding {
                mode: AxisMode::Keys,
                positive: Some(pad("BTN_TL")),
                threshold: 0.7,
                ..Default::default()
            },
        );
        let a = step(
            a.state,
            &p,
            Input::Axis {
                code: "ABS_Z".into(),
                value: 0.8,
            },
            1,
        );
        assert_eq!(
            a.output,
            vec![Output::Button {
                target: pad("BTN_TL"),
                pressed: true
            }]
        );
        assert!(p.validate().is_ok());
    }
}
