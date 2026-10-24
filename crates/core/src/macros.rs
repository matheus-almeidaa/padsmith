use super::*;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Macro {
    pub name: String,
    pub steps: Vec<MacroStep>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum MacroStep {
    Press { target: Target },
    Release { target: Target },
    Hold { target: Target, ms: u64 },
    Wait { ms: u64 },
    Repeat { count: u32, steps: Vec<MacroStep> },
    Axis { code: String, value: f64 },
    Move { x: i32, y: i32 },
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TriggerKind {
    #[default]
    Tap,
    LongPress,
    DoubleTap,
    Chord,
    WhileHeld,
    ToggleLoop,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct MacroTrigger {
    pub id: String,
    pub buttons: Vec<String>,
    pub macro_id: String,
    pub kind: TriggerKind,
    pub threshold_ms: u64,
    pub window_ms: u64,
    pub consume: bool,
}
impl Default for MacroTrigger {
    fn default() -> Self {
        Self {
            id: String::new(),
            buttons: Vec::new(),
            macro_id: String::new(),
            kind: TriggerKind::Tap,
            threshold_ms: 500,
            window_ms: 250,
            consume: true,
        }
    }
}
#[derive(Clone, Debug)]
enum Action {
    Press(Target),
    Release(Target),
    Axis(String, f64),
    Move(i32, i32),
}
#[derive(Clone, Debug)]
pub(crate) struct Run {
    actions: Vec<(u64, Action)>,
    duration: u64,
    start: u64,
    index: usize,
    looping: bool,
}
#[derive(Clone, Debug, Default)]
pub(crate) struct Gesture {
    down: bool,
    since: u64,
    fired: bool,
    last_tap: Option<u64>,
}
#[derive(Clone, Debug)]
pub(crate) struct Turbo {
    pub target: Option<Target>,
    pub start: u64,
    pub half_period: u64,
    pub next: u64,
}

fn flatten(
    steps: &[MacroStep],
    at: &mut u64,
    out: &mut Vec<(u64, Action)>,
    depth: u8,
    budget: &mut usize,
) -> Result<(), String> {
    if depth > 8 {
        return Err("Macros podem aninhar até 8 repetições".into());
    }
    for step in steps {
        if *budget == 0 {
            return Err("Macro excede o limite de complexidade".into());
        }
        *budget -= 1;
        if out.len() > 4096 {
            return Err("Macro excede 4096 ações expandidas".into());
        }
        match step {
            MacroStep::Press { target } => out.push((*at, Action::Press(target.clone()))),
            MacroStep::Release { target } => out.push((*at, Action::Release(target.clone()))),
            MacroStep::Hold { target, ms } => {
                if *ms == 0 || *ms > 600_000 {
                    return Err("Duração de segurar deve estar entre 1 e 600000 ms".into());
                }
                out.push((*at, Action::Press(target.clone())));
                *at += ms * 1000;
                out.push((*at, Action::Release(target.clone())));
            }
            MacroStep::Wait { ms } => {
                if *ms > 600_000 {
                    return Err("Espera excede 600000 ms".into());
                }
                *at += ms * 1000;
            }
            MacroStep::Repeat { count, steps } => {
                if *count == 0 || *count > 1000 || steps.is_empty() {
                    return Err("Repetição deve ter passos e contagem de 1 a 1000".into());
                }
                for _ in 0..*count {
                    flatten(steps, at, out, depth + 1, budget)?;
                }
            }
            MacroStep::Axis { code, value } => {
                if !GAMEPAD_AXES.contains(&code.as_str())
                    || !value.is_finite()
                    || !(-1.0..=1.0).contains(value)
                {
                    return Err("Eixo de macro inválido".into());
                }
                out.push((*at, Action::Axis(code.clone(), *value)));
            }
            MacroStep::Move { x, y } => {
                if x.unsigned_abs() > 10000 || y.unsigned_abs() > 10000 {
                    return Err("Movimento de mouse excede 10000 pixels".into());
                }
                out.push((*at, Action::Move(*x, *y)));
            }
        }
    }
    if out.len() > 4096 {
        return Err("Macro excede 4096 ações expandidas".into());
    }
    Ok(())
}
pub fn targets(steps: &[MacroStep]) -> Vec<&Target> {
    steps
        .iter()
        .flat_map(|s| match s {
            MacroStep::Press { target }
            | MacroStep::Release { target }
            | MacroStep::Hold { target, .. } => vec![target],
            MacroStep::Repeat { steps, .. } => targets(steps),
            _ => Vec::new(),
        })
        .collect()
}
pub fn validate(profile: &Profile) -> Result<(), String> {
    let mut durations = BTreeMap::new();
    for (id, sequence) in &profile.macros {
        let mut time = 0;
        let mut actions = Vec::new();
        flatten(&sequence.steps, &mut time, &mut actions, 0, &mut 8192)?;
        if actions.is_empty() {
            return Err(format!("Macro {id} não contém ações"));
        }
        durations.insert(id, time);
    }
    let mut ids = BTreeSet::new();
    for t in &profile.triggers {
        if t.id.is_empty() || !ids.insert(&t.id) {
            return Err("Gatilhos precisam de IDs únicos".into());
        }
        if t.buttons.is_empty()
            || t.buttons.len() > 8
            || t.buttons.iter().collect::<BTreeSet<_>>().len() != t.buttons.len()
        {
            return Err("Gatilho deve ter de 1 a 8 botões distintos".into());
        }
        if t.kind == TriggerKind::Chord && t.buttons.len() < 2 {
            return Err("Chord precisa de pelo menos dois botões".into());
        }
        let duration = durations
            .get(&t.macro_id)
            .ok_or_else(|| format!("Macro ausente: {}", t.macro_id))?;
        if matches!(t.kind, TriggerKind::WhileHeld | TriggerKind::ToggleLoop) && *duration == 0 {
            return Err("Loops precisam conter pelo menos 1 ms de duração".into());
        }
        if !(1..=60_000).contains(&t.threshold_ms) || !(1..=5000).contains(&t.window_ms) {
            return Err("Tempo de gatilho fora dos limites".into());
        }
    }
    Ok(())
}
fn cancel(state: &mut State, id: &str, output: &mut Vec<Output>) {
    state.runs.remove(id);
    let owner = format!("macro:{id}");
    state.owners.remove(&owner);
    state.release_axis_owner(&owner, output);
}
fn start(
    state: &mut State,
    profile: &Profile,
    trigger: &MacroTrigger,
    now: u64,
    output: &mut Vec<Output>,
) {
    cancel(state, &trigger.id, output);
    if let Some(sequence) = profile.macros.get(&trigger.macro_id) {
        let mut duration = 0;
        let mut actions = Vec::new();
        if flatten(&sequence.steps, &mut duration, &mut actions, 0, &mut 8192).is_ok() {
            state.runs.insert(
                trigger.id.clone(),
                Run {
                    actions,
                    duration,
                    start: now,
                    index: 0,
                    looping: matches!(
                        trigger.kind,
                        TriggerKind::WhileHeld | TriggerKind::ToggleLoop
                    ),
                },
            );
        }
    }
}
pub(crate) fn play(
    state: &mut State,
    profile: &Profile,
    macro_id: &str,
    now: u64,
    output: &mut Vec<Output>,
) {
    start(
        state,
        profile,
        &MacroTrigger {
            id: "preview".into(),
            macro_id: macro_id.into(),
            ..Default::default()
        },
        now,
        output,
    );
}
pub(crate) fn gestures(
    state: &mut State,
    profile: &Profile,
    now: u64,
    before: &mut BTreeSet<Target>,
    output: &mut Vec<Output>,
) {
    for t in &profile.triggers {
        let down = t.buttons.iter().all(|b| state.down.contains(b));
        let mut gesture = state.gestures.remove(&t.id).unwrap_or_default();
        if down && !gesture.down {
            gesture.since = now;
            gesture.fired = false;
            match t.kind {
                TriggerKind::Chord | TriggerKind::WhileHeld => {
                    start(state, profile, t, now, output)
                }
                TriggerKind::ToggleLoop => {
                    if state.runs.contains_key(&t.id) {
                        cancel(state, &t.id, output);
                    } else {
                        start(state, profile, t, now, output);
                    }
                }
                TriggerKind::DoubleTap
                    if gesture
                        .last_tap
                        .is_some_and(|last| now.saturating_sub(last) <= t.window_ms * 1000) =>
                {
                    start(state, profile, t, now, output);
                    gesture.last_tap = None;
                    gesture.fired = true;
                }
                _ => {}
            }
        }
        if !down && gesture.down {
            let short = now.saturating_sub(gesture.since) <= t.threshold_ms * 1000;
            match t.kind {
                TriggerKind::Tap if short => start(state, profile, t, now, output),
                TriggerKind::DoubleTap if short && !gesture.fired => gesture.last_tap = Some(now),
                TriggerKind::WhileHeld => cancel(state, &t.id, output),
                _ => {}
            }
        }
        if down
            && t.kind == TriggerKind::LongPress
            && !gesture.fired
            && now >= gesture.since + t.threshold_ms * 1000
        {
            start(state, profile, t, now, output);
            gesture.fired = true;
        }
        gesture.down = down;
        state.gestures.insert(t.id.clone(), gesture);
        flush_buttons(before, state, output);
    }
}
pub(crate) fn advance(
    state: &mut State,
    now: u64,
    before: &mut BTreeSet<Target>,
    output: &mut Vec<Output>,
) {
    for code in state.turbo.keys().cloned().collect::<Vec<_>>() {
        let turbo = state.turbo.get_mut(&code).unwrap();
        if now >= turbo.next {
            let phase = now.saturating_sub(turbo.start) / turbo.half_period;
            turbo.next = turbo.start + (phase + 1) * turbo.half_period;
            let target = if phase.is_multiple_of(2) {
                turbo.target.clone()
            } else {
                None
            };
            state.own(&format!("button:{code}"), target);
            flush_buttons(before, state, output);
        }
    }
    let mut budget = 512;
    for id in state.runs.keys().cloned().collect::<Vec<_>>() {
        let mut run = state.runs.remove(&id).unwrap();
        let owner = format!("macro:{id}");
        while budget > 0
            && run.index < run.actions.len()
            && run.start + run.actions[run.index].0 <= now
        {
            let action = &run.actions[run.index].1;
            match action {
                Action::Press(target) if *target != Target::Disabled => {
                    state
                        .owners
                        .entry(owner.clone())
                        .or_default()
                        .insert(target.clone());
                }
                Action::Release(target) => {
                    if let Some(set) = state.owners.get_mut(&owner) {
                        set.remove(target);
                    }
                }
                Action::Axis(code, value) => state.axis(&owner, code, Some(*value), output),
                Action::Move(x, y) => {
                    if *x != 0 {
                        output.push(Output::Relative {
                            code: "REL_X".into(),
                            value: *x,
                        });
                    }
                    if *y != 0 {
                        output.push(Output::Relative {
                            code: "REL_Y".into(),
                            value: *y,
                        });
                    }
                }
                _ => {}
            }
            run.index += 1;
            budget -= 1;
            flush_buttons(before, state, output);
        }
        if run.index == run.actions.len() && now >= run.start + run.duration {
            state.owners.remove(&owner);
            state.release_axis_owner(&owner, output);
            flush_buttons(before, state, output);
            if run.looping && run.duration > 0 {
                run.start += ((now.saturating_sub(run.start) / run.duration).max(1)) * run.duration;
                run.index = 0;
                state.runs.insert(id, run);
            }
        } else {
            state.runs.insert(id, run);
        }
    }
}
pub(crate) fn deadline(state: &State, profile: &Profile, now: u64) -> Option<u64> {
    if state.paused {
        return None;
    }
    let macro_times = state.runs.values().map(|r| {
        r.actions
            .get(r.index)
            .map(|(at, _)| r.start + at)
            .unwrap_or(r.start + r.duration)
    });
    let gestures = profile
        .triggers
        .iter()
        .filter(|t| t.kind == TriggerKind::LongPress)
        .filter_map(|t| {
            let g = state.gestures.get(&t.id)?;
            (g.down && !g.fired).then_some(g.since + t.threshold_ms * 1000)
        });
    macro_times
        .chain(gestures)
        .chain(state.turbo.values().map(|t| t.next))
        .min()
        .map(|d| d.max(now))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Recording {
    pub sequence: Macro,
    pub omitted: Vec<String>,
}
#[derive(Clone, Debug)]
pub struct Recorder {
    last: u64,
    steps: Vec<MacroStep>,
    held: BTreeSet<Target>,
    axes: BTreeSet<String>,
    omitted: BTreeSet<String>,
    owners: BTreeMap<String, Target>,
}
impl Recorder {
    pub fn new(now: u64) -> Self {
        Self {
            last: now,
            steps: Vec::new(),
            held: BTreeSet::new(),
            axes: BTreeSet::new(),
            omitted: BTreeSet::new(),
            owners: BTreeMap::new(),
        }
    }
    pub fn push(&mut self, profile: &Profile, input: &Input, now: u64) {
        if self.steps.len() >= 3800 {
            self.omitted
                .insert("Limite de 3800 passos atingido; encerre a gravação".into());
            return;
        }
        let step = match input {
            Input::Button { code, pressed } => {
                if !pressed {
                    let Some(target) = self.owners.remove(code) else {
                        return;
                    };
                    if self.owners.values().any(|other| *other == target) {
                        return;
                    }
                    self.held.remove(&target);
                    self.wait(now);
                    self.steps.push(MacroStep::Release { target });
                    return;
                }
                if self.owners.contains_key(code) {
                    return;
                }
                let target = profile
                    .buttons
                    .get(code)
                    .and_then(|b| b.output.clone())
                    .or_else(|| default_target(code));
                let Some(target) = target.filter(|t| *t != Target::Disabled) else {
                    self.omitted.insert(code.clone());
                    return;
                };
                self.owners.insert(code.clone(), target.clone());
                if *pressed {
                    if !self.held.insert(target.clone()) {
                        return;
                    }
                    MacroStep::Press { target }
                } else {
                    if !self.held.remove(&target) {
                        return;
                    }
                    MacroStep::Release { target }
                }
            }
            Input::Axis { code, value } if GAMEPAD_AXES.contains(&code.as_str()) => {
                self.axes.insert(code.clone());
                MacroStep::Axis {
                    code: code.clone(),
                    value: *value,
                }
            }
            Input::Axis { code, .. } => {
                self.omitted.insert(code.clone());
                return;
            }
            _ => return,
        };
        self.wait(now);
        self.steps.push(step);
    }
    fn wait(&mut self, now: u64) {
        let ms = now.saturating_sub(self.last) / 1000;
        if ms > 0 {
            self.steps.push(MacroStep::Wait {
                ms: ms.min(600_000),
            });
            self.last += ms * 1000;
        }
    }
    pub fn finish(mut self, now: u64) -> Recording {
        self.wait(now);
        for target in self.held {
            self.steps.push(MacroStep::Release { target });
        }
        for code in self.axes {
            self.steps.push(MacroStep::Axis { code, value: 0.0 });
        }
        Recording {
            sequence: Macro {
                name: "Gravação do controle".into(),
                steps: self.steps,
            },
            omitted: self.omitted.into_iter().collect(),
        }
    }
}
