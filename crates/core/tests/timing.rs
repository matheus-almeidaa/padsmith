use padsmith_core::*;
fn pad(code: &str) -> Target {
    Target::Gamepad { code: code.into() }
}
fn key(code: &str) -> Target {
    Target::Key { code: code.into() }
}
fn button(code: &str, pressed: bool) -> Input {
    Input::Button {
        code: code.into(),
        pressed,
    }
}
fn edge(target: Target, pressed: bool) -> Output {
    Output::Button { target, pressed }
}
fn profile(kind: TriggerKind, buttons: &[&str], steps: Vec<MacroStep>) -> Profile {
    let mut p = Profile::default();
    p.macros.insert(
        "sequence".into(),
        Macro {
            name: "Teste".into(),
            steps,
        },
    );
    p.triggers.push(MacroTrigger {
        id: "trigger".into(),
        buttons: buttons.iter().map(|s| (*s).into()).collect(),
        macro_id: "sequence".into(),
        kind,
        ..Default::default()
    });
    p.validate().unwrap();
    p
}
#[test]
fn turbo_uses_deadlines_and_skips_missed_edges_without_sticking() {
    let mut p = Profile::default();
    p.buttons.insert(
        "BTN_SOUTH".into(),
        Binding {
            mode: ButtonMode::Turbo,
            hz: 20.0,
            ..Default::default()
        },
    );
    let t = step(State::default(), &p, button("BTN_SOUTH", true), 0);
    assert_eq!(t.output, vec![edge(pad("BTN_SOUTH"), true)]);
    assert_eq!(t.next_deadline, Some(25_000));
    let t = step(t.state, &p, Input::Tick, 25_000);
    assert_eq!(t.output, vec![edge(pad("BTN_SOUTH"), false)]);
    let t = step(t.state, &p, Input::Tick, 100_000);
    assert_eq!(t.output, vec![edge(pad("BTN_SOUTH"), true)]);
    let t = step(t.state, &p, button("BTN_SOUTH", false), 100_001);
    assert_eq!(t.output, vec![edge(pad("BTN_SOUTH"), false)]);
    assert_eq!(t.next_deadline, None);
}
#[test]
fn macro_timeline_repeats_and_ownership_preserve_physical_hold() {
    let p = profile(
        TriggerKind::Tap,
        &["BTN_WEST"],
        vec![MacroStep::Repeat {
            count: 2,
            steps: vec![
                MacroStep::Hold {
                    target: pad("BTN_SOUTH"),
                    ms: 10,
                },
                MacroStep::Wait { ms: 10 },
            ],
        }],
    );
    let t = step(State::default(), &p, button("BTN_SOUTH", true), 0);
    let t = step(t.state, &p, button("BTN_WEST", true), 1);
    let t = step(t.state, &p, button("BTN_WEST", false), 2);
    assert!(t.output.is_empty());
    assert_eq!(t.next_deadline, Some(10_002));
    let t = step(t.state, &p, Input::Tick, 10_002);
    assert!(t.output.is_empty());
    let t = step(t.state, &p, Input::Tick, 20_002);
    assert!(t.output.is_empty());
    let t = step(t.state, &p, Input::Tick, 40_002);
    assert!(t.output.is_empty());
    let t = step(t.state, &p, button("BTN_SOUTH", false), 40_003);
    assert_eq!(t.output, vec![edge(pad("BTN_SOUTH"), false)]);
    assert_eq!(t.next_deadline, None);
}
#[test]
fn zero_time_press_release_is_not_collapsed() {
    let p = profile(
        TriggerKind::Chord,
        &["BTN_TL", "BTN_TR"],
        vec![
            MacroStep::Press {
                target: key("KEY_E"),
            },
            MacroStep::Release {
                target: key("KEY_E"),
            },
        ],
    );
    let t = step(State::default(), &p, button("BTN_TL", true), 0);
    assert!(t.output.is_empty());
    let t = step(t.state, &p, button("BTN_TR", true), 1);
    assert_eq!(
        t.output,
        vec![edge(key("KEY_E"), true), edge(key("KEY_E"), false)]
    );
}
#[test]
fn long_and_double_tap_have_deterministic_thresholds() {
    let p = profile(
        TriggerKind::LongPress,
        &["BTN_WEST"],
        vec![MacroStep::Hold {
            target: key("KEY_SPACE"),
            ms: 10,
        }],
    );
    let t = step(State::default(), &p, button("BTN_WEST", true), 0);
    assert_eq!(t.next_deadline, Some(500_000));
    let t = step(t.state, &p, Input::Tick, 499_999);
    assert!(t.output.is_empty());
    let t = step(t.state, &p, Input::Tick, 500_000);
    assert_eq!(t.output, vec![edge(key("KEY_SPACE"), true)]);
    let t = step(t.state, &p, Input::Reset, 500_001);
    assert_eq!(t.output, vec![edge(key("KEY_SPACE"), false)]);
    assert_eq!(t.next_deadline, None);
    let p = profile(
        TriggerKind::DoubleTap,
        &["BTN_WEST"],
        vec![MacroStep::Hold {
            target: pad("BTN_EAST"),
            ms: 20,
        }],
    );
    let t = step(State::default(), &p, button("BTN_WEST", true), 0);
    let t = step(t.state, &p, button("BTN_WEST", false), 50_000);
    assert!(t.output.is_empty());
    let t = step(t.state, &p, button("BTN_WEST", true), 200_000);
    assert_eq!(t.output, vec![edge(pad("BTN_EAST"), true)]);
}
#[test]
fn held_and_toggle_loops_cancel_every_owned_output() {
    for kind in [TriggerKind::WhileHeld, TriggerKind::ToggleLoop] {
        let p = profile(
            kind.clone(),
            &["BTN_WEST"],
            vec![
                MacroStep::Press {
                    target: key("KEY_W"),
                },
                MacroStep::Axis {
                    code: "ABS_X".into(),
                    value: 1.0,
                },
                MacroStep::Wait { ms: 10 },
            ],
        );
        let t = step(State::default(), &p, button("BTN_WEST", true), 0);
        assert!(t.output.contains(&edge(key("KEY_W"), true)));
        let mut t = step(t.state, &p, button("BTN_WEST", false), 1);
        if kind == TriggerKind::ToggleLoop {
            assert!(t.output.is_empty());
            t = step(t.state, &p, button("BTN_WEST", true), 2);
        }
        assert!(t.output.contains(&edge(key("KEY_W"), false)));
        assert!(t.output.contains(&Output::Axis {
            code: "ABS_X".into(),
            value: 0.0
        }));
        assert_eq!(t.next_deadline, None);
    }
}
#[test]
fn recorder_preserves_timing_releases_holds_and_reports_unknown_inputs() {
    let p = Profile::default();
    let mut recorder = Recorder::new(0);
    recorder.push(&p, &button("BTN_SOUTH", true), 10_000);
    recorder.push(&p, &button("aux:KEY_F13", true), 15_000);
    recorder.push(
        &p,
        &Input::Axis {
            code: "ABS_X".into(),
            value: 0.8,
        },
        20_000,
    );
    let recording = recorder.finish(30_000);
    assert_eq!(recording.omitted, vec!["aux:KEY_F13"]);
    assert!(matches!(
        recording.sequence.steps[0],
        MacroStep::Wait { ms: 10 }
    ));
    assert!(matches!(
        recording.sequence.steps[3],
        MacroStep::Axis { value: 0.8, .. }
    ));
    assert!(
        recording
            .sequence
            .steps
            .iter()
            .any(|s| matches!(s,MacroStep::Release {target} if *target==pad("BTN_SOUTH")))
    );
    assert!(matches!(
        recording.sequence.steps.last(),
        Some(MacroStep::Axis { value: 0.0, .. })
    ));
}
#[test]
fn invalid_instant_loops_and_pathological_repeats_are_rejected() {
    let mut p = profile(
        TriggerKind::Tap,
        &["BTN_WEST"],
        vec![MacroStep::Press {
            target: pad("BTN_SOUTH"),
        }],
    );
    p.triggers[0].kind = TriggerKind::ToggleLoop;
    assert!(p.validate().is_err());
    let mut steps = vec![MacroStep::Wait { ms: 1 }];
    for _ in 0..6 {
        steps = vec![MacroStep::Repeat { count: 1000, steps }];
    }
    p.macros.get_mut("sequence").unwrap().steps = steps;
    assert!(p.validate().is_err());
}
