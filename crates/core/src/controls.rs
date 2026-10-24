use super::*;
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Layer {
    pub name: String,
    pub activation: Vec<String>,
    pub priority: i32,
    pub buttons: BTreeMap<String, Binding>,
    pub axes: BTreeMap<String, AxisBinding>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Emergency {
    pub enabled: bool,
    pub buttons: Vec<String>,
    pub hold_ms: u64,
}
impl Default for Emergency {
    fn default() -> Self {
        Self {
            enabled: true,
            buttons: vec!["BTN_MODE".into(), "BTN_SELECT".into()],
            hold_ms: 3000,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ProfileShortcut {
    pub buttons: Vec<String>,
    pub profile: String,
    pub hold_ms: u64,
}
impl Default for ProfileShortcut {
    fn default() -> Self {
        Self {
            buttons: Vec::new(),
            profile: "$next".into(),
            hold_ms: 700,
        }
    }
}
#[derive(Clone, Debug, Default)]
pub(crate) struct Gesture {
    down: bool,
    since: u64,
    fired: bool,
}

fn combination(buttons: &[String]) -> bool {
    !buttons.is_empty()
        && buttons.len() <= 8
        && buttons.iter().collect::<BTreeSet<_>>().len() == buttons.len()
}
pub(crate) fn validate(p: &Profile) -> Result<(), String> {
    if p.emergency.enabled
        && (!combination(&p.emergency.buttons) || !(1..=60000).contains(&p.emergency.hold_ms))
    {
        return Err("Atalho de emergência inválido".into());
    }
    for shortcut in &p.shortcuts {
        if !combination(&shortcut.buttons)
            || shortcut.profile.is_empty()
            || !(1..=60000).contains(&shortcut.hold_ms)
        {
            return Err("Atalho de perfil inválido".into());
        }
    }
    for (id, layer) in &p.layers {
        if id.is_empty() || !combination(&layer.activation) {
            return Err("Camada exige nome e combinação de ativação válida".into());
        }
        let layer_profile = Profile {
            buttons: layer.buttons.clone(),
            axes: layer.axes.clone(),
            ..Default::default()
        };
        layer_profile.validate()?;
    }
    if p.processes
        .iter()
        .any(|s| s.is_empty() || s.len() > 256 || s.contains('/') || s.contains('\\'))
    {
        return Err("Processos devem ser nomes/globs de executáveis, sem diretórios".into());
    }
    Ok(())
}
fn clear(state: &mut State, output: &mut Vec<Output>) {
    for (code, value) in &state.axes {
        if *value != 0.0 {
            output.push(Output::Axis {
                code: code.clone(),
                value: 0.0,
            });
        }
    }
    state.owners.clear();
    state.axes.clear();
    state.axis_owners.clear();
    state.turbo.clear();
    state.runs.clear();
    state.mouse.clear();
    state.mouse_tick = None;
    state.remainder.clear();
}
fn button_binding<'a>(state: &State, p: &'a Profile, code: &str) -> Option<&'a Binding> {
    if state.paused {
        return None;
    }
    state
        .active_layers
        .iter()
        .rev()
        .find_map(|id| p.layers.get(id)?.buttons.get(code))
        .or_else(|| p.buttons.get(code))
}
pub(crate) fn button(
    state: &mut State,
    p: &Profile,
    code: &str,
    pressed: bool,
    edge: bool,
    now: u64,
) {
    let binding = button_binding(state, p, code);
    let consumed = !state.paused
        && (p
            .triggers
            .iter()
            .any(|t| t.consume && t.buttons.iter().any(|b| b == code))
            || p.layers
                .values()
                .any(|l| l.activation.iter().any(|b| b == code))
            || p.shortcuts
                .iter()
                .any(|s| s.buttons.iter().any(|b| b == code)));
    let target = binding
        .and_then(|b| b.output.clone())
        .or_else(|| default_target(code));
    let owner = format!("button:{code}");
    if consumed {
        state.own(&owner, None);
        state.turbo.remove(code);
    } else if binding.is_some_and(|b| b.mode == ButtonMode::Toggle) {
        if edge && pressed {
            if state.owners.contains_key(&owner) {
                state.own(&owner, None);
            } else {
                state.own(&owner, target);
            }
        }
    } else if let Some(b) = binding.filter(|b| b.mode == ButtonMode::Turbo) {
        if pressed {
            let half_period = (500_000.0 / b.hz).round() as u64;
            state.own(&owner, target.clone());
            state.turbo.insert(
                code.into(),
                macros::Turbo {
                    target,
                    start: now,
                    half_period,
                    next: now + half_period,
                },
            );
        } else {
            state.turbo.remove(code);
            state.own(&owner, None);
        }
    } else {
        state.own(&owner, if pressed { target } else { None });
    }
}
pub(crate) fn axis(
    state: &mut State,
    p: &Profile,
    code: &str,
    value: f64,
    now: u64,
    output: &mut Vec<Output>,
) {
    let binding = if state.paused {
        None
    } else {
        state
            .active_layers
            .iter()
            .rev()
            .find_map(|id| p.layers.get(id)?.axes.get(code))
            .or_else(|| p.axes.get(code))
    };
    let defaults = AxisBinding::default();
    let b = binding.unwrap_or(&defaults);
    let v = value.clamp(-1.0, 1.0);
    let v = if v.abs() <= b.deadzone {
        0.0
    } else {
        v.signum() * ((v.abs() - b.deadzone) / (1.0 - b.deadzone)).powf(b.curve)
    };
    let v = if b.invert {
        if ["ABS_Z", "ABS_RZ"].contains(&code) {
            1.0 - v
        } else {
            -v
        }
    } else {
        v
    };
    match b.mode {
        AxisMode::Gamepad => {
            let target = b.target.as_deref().unwrap_or(code);
            if GAMEPAD_AXES.contains(&target) {
                state.axis(&format!("axis:{code}"), target, Some(v), output);
            }
        }
        AxisMode::Keys => {
            state.own(
                &format!("axis:{code}-"),
                if v <= -b.threshold {
                    b.negative.clone()
                } else {
                    None
                },
            );
            state.own(
                &format!("axis:{code}+"),
                if v >= b.threshold {
                    b.positive.clone()
                } else {
                    None
                },
            );
        }
        AxisMode::Mouse => {
            if v == 0.0 {
                state.mouse.remove(code);
            } else {
                state.mouse.insert(
                    code.into(),
                    (b.target.clone().unwrap_or_default(), v * b.sensitivity),
                );
            }
            if state.mouse.is_empty() {
                state.mouse_tick = None;
                state.remainder.clear();
            } else if state.mouse_tick.is_none() {
                state.mouse_tick = Some(now);
            }
        }
    }
}
pub(crate) fn layers(
    state: &mut State,
    p: &Profile,
    now: u64,
    output: &mut Vec<Output>,
    effects: &mut Vec<Effect>,
) {
    let mut active: Vec<_> = p
        .layers
        .iter()
        .filter(|(_, l)| !state.paused && l.activation.iter().all(|b| state.down.contains(b)))
        .collect();
    active.sort_by(|(a, x), (b, y)| x.priority.cmp(&y.priority).then(a.cmp(b)));
    let active: Vec<_> = active.into_iter().map(|(id, _)| id.clone()).collect();
    if active == state.active_layers {
        return;
    }
    clear(state, output);
    state.active_layers = active;
    // Held normal buttons/axes adopt the new layer; toggles don't retrigger.
    for code in state.down.clone() {
        button(state, p, &code, true, false, now);
    }
    for (code, value) in state.raw_axes.clone() {
        axis(state, p, &code, value, now, output);
    }
    effects.push(Effect::LayersChanged);
}
fn update(gesture: &mut Gesture, down: bool, now: u64) {
    if down && !gesture.down {
        gesture.since = now;
        gesture.fired = false;
    }
    if !down {
        gesture.fired = false;
    }
    gesture.down = down;
}
pub(crate) fn advance(
    state: &mut State,
    p: &Profile,
    now: u64,
    output: &mut Vec<Output>,
    effects: &mut Vec<Effect>,
) {
    let down = p.emergency.enabled && p.emergency.buttons.iter().all(|b| state.down.contains(b));
    update(&mut state.emergency_gesture, down, now);
    if down
        && !state.emergency_gesture.fired
        && now >= state.emergency_gesture.since + p.emergency.hold_ms * 1000
    {
        clear(state, output);
        state.paused = !state.paused;
        state.emergency_gesture.fired = true;
        state.active_layers.clear();
        state.gestures.clear();
        state.shortcut_gestures.clear();
        effects.push(Effect::PauseChanged {
            paused: state.paused,
        });
        return;
    }
    if state.paused {
        return;
    }
    for (index, shortcut) in p.shortcuts.iter().enumerate() {
        let down = shortcut.buttons.iter().all(|b| state.down.contains(b));
        let gesture = state.shortcut_gestures.entry(index).or_default();
        update(gesture, down, now);
        if down && !gesture.fired && now >= gesture.since + shortcut.hold_ms * 1000 {
            gesture.fired = true;
            effects.push(Effect::SelectProfile {
                profile: shortcut.profile.clone(),
            });
        }
    }
}
pub(crate) fn deadline(state: &State, p: &Profile) -> Option<u64> {
    let emergency = (state.emergency_gesture.down && !state.emergency_gesture.fired)
        .then_some(state.emergency_gesture.since + p.emergency.hold_ms * 1000);
    let shortcuts = state
        .shortcut_gestures
        .iter()
        .filter(|(_, g)| !state.paused && g.down && !g.fired)
        .filter_map(|(i, g)| p.shortcuts.get(*i).map(|s| g.since + s.hold_ms * 1000));
    emergency.into_iter().chain(shortcuts).min()
}
