use padsmith_core::*;
fn pad(code: &str) -> Target {
    Target::Gamepad { code: code.into() }
}
fn edge(code: &str, pressed: bool) -> Input {
    Input::Button {
        code: code.into(),
        pressed,
    }
}
fn out(target: Target, pressed: bool) -> Output {
    Output::Button { target, pressed }
}
#[test]
fn layer_adopts_held_buttons_and_releases_old_destinations() {
    let mut p = Profile::default();
    let mut layer = Layer {
        name: "Camada traseira".into(),
        activation: vec!["aux:KEY_F13".into()],
        ..Default::default()
    };
    layer.buttons.insert(
        "BTN_SOUTH".into(),
        Binding {
            output: Some(Target::Key {
                code: "KEY_W".into(),
            }),
            ..Default::default()
        },
    );
    p.layers.insert("back".into(), layer);
    p.validate().unwrap();
    let t = step(State::default(), &p, edge("BTN_SOUTH", true), 0);
    assert_eq!(t.output, vec![out(pad("BTN_SOUTH"), true)]);
    let t = step(t.state, &p, edge("aux:KEY_F13", true), 1);
    assert_eq!(
        t.output,
        vec![
            out(pad("BTN_SOUTH"), false),
            out(
                Target::Key {
                    code: "KEY_W".into()
                },
                true
            )
        ]
    );
    assert_eq!(t.state.active_layers(), &["back".to_string()]);
    let t = step(t.state, &p, edge("aux:KEY_F13", false), 2);
    assert!(t.output.contains(&out(
        Target::Key {
            code: "KEY_W".into()
        },
        false
    )));
    assert!(t.output.contains(&out(pad("BTN_SOUTH"), true)));
    assert_eq!(
        step(t.state, &p, edge("BTN_SOUTH", false), 3).output,
        vec![out(pad("BTN_SOUTH"), false)]
    );
}
#[test]
fn layer_neutralizes_and_restores_held_axes() {
    let mut p = Profile::default();
    let mut l = Layer {
        activation: vec!["BTN_THUMBL".into()],
        ..Default::default()
    };
    l.axes.insert(
        "ABS_X".into(),
        AxisBinding {
            target: Some("ABS_RX".into()),
            invert: true,
            ..Default::default()
        },
    );
    p.layers.insert("aim".into(), l);
    let t = step(
        State::default(),
        &p,
        Input::Axis {
            code: "ABS_X".into(),
            value: 0.5,
        },
        0,
    );
    let t = step(t.state, &p, edge("BTN_THUMBL", true), 1);
    assert_eq!(
        t.output,
        vec![
            Output::Axis {
                code: "ABS_X".into(),
                value: 0.0
            },
            Output::Axis {
                code: "ABS_RX".into(),
                value: -0.5
            }
        ]
    );
    let t = step(t.state, &p, edge("BTN_THUMBL", false), 2);
    assert!(t.output.contains(&Output::Axis {
        code: "ABS_RX".into(),
        value: 0.0
    }));
    assert!(t.output.contains(&Output::Axis {
        code: "ABS_X".into(),
        value: 0.5
    }));
}
#[test]
fn emergency_neutralizes_toggle_and_rearms_only_after_release() {
    let mut p = Profile::default();
    p.buttons.insert(
        "BTN_SOUTH".into(),
        Binding {
            mode: ButtonMode::Toggle,
            ..Default::default()
        },
    );
    let t = step(State::default(), &p, edge("BTN_SOUTH", true), 0);
    let t = step(t.state, &p, edge("BTN_SOUTH", false), 1);
    let t = step(t.state, &p, edge("BTN_MODE", true), 2);
    let t = step(t.state, &p, edge("BTN_SELECT", true), 3);
    assert_eq!(t.next_deadline, Some(3_000_003));
    let t = step(t.state, &p, Input::Tick, 3_000_003);
    assert!(t.state.paused);
    assert!(t.output.contains(&out(pad("BTN_SOUTH"), false)));
    assert!(t.effects.contains(&Effect::PauseChanged { paused: true }));
    let t = step(t.state, &p, Input::Tick, 9_000_000);
    assert!(t.state.paused);
    assert!(t.effects.is_empty());
    let t = step(t.state, &p, edge("BTN_SELECT", false), 9_000_001);
    let t = step(t.state, &p, edge("BTN_SELECT", true), 9_000_002);
    let t = step(t.state, &p, Input::Tick, 12_000_002);
    assert!(!t.state.paused);
    assert!(t.effects.contains(&Effect::PauseChanged { paused: false }));
}
#[test]
fn profile_shortcut_is_timed_consumed_and_fires_once() {
    let mut p = Profile::default();
    p.shortcuts.push(ProfileShortcut {
        buttons: vec!["BTN_TL".into(), "BTN_START".into()],
        profile: "$next".into(),
        hold_ms: 500,
    });
    let t = step(State::default(), &p, edge("BTN_TL", true), 0);
    assert!(t.output.is_empty());
    let t = step(t.state, &p, edge("BTN_START", true), 1);
    assert!(t.output.is_empty());
    assert_eq!(t.next_deadline, Some(500_001));
    let t = step(t.state, &p, Input::Tick, 500_001);
    assert_eq!(
        t.effects,
        vec![Effect::SelectProfile {
            profile: "$next".into()
        }]
    );
    assert!(step(t.state, &p, Input::Tick, 1_000_000).effects.is_empty());
}
