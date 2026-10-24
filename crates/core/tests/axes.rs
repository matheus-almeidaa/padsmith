use padsmith_core::*;
fn button(code: &str, pressed: bool) -> Input {
    Input::Button {
        code: code.into(),
        pressed,
    }
}
#[test]
fn digital_triggers_and_analog_triggers_share_ownership() {
    let p = Profile::default();
    let t = step(State::default(), &p, button("BTN_TL2", true), 0);
    assert_eq!(
        t.output,
        vec![Output::Axis {
            code: "ABS_Z".into(),
            value: 1.0
        }]
    );
    let t = step(
        t.state,
        &p,
        Input::Axis {
            code: "ABS_Z".into(),
            value: 0.4,
        },
        1,
    );
    assert!(t.output.is_empty());
    let t = step(t.state, &p, button("BTN_TL2", false), 2);
    assert_eq!(
        t.output,
        vec![Output::Axis {
            code: "ABS_Z".into(),
            value: 0.4
        }]
    );
}
#[test]
fn digital_dpad_restores_other_held_direction() {
    let p = Profile::default();
    let t = step(State::default(), &p, button("BTN_DPAD_UP", true), 0);
    assert_eq!(
        t.output,
        vec![Output::Axis {
            code: "ABS_HAT0Y".into(),
            value: -1.0
        }]
    );
    let t = step(t.state, &p, button("BTN_DPAD_DOWN", true), 1);
    assert_eq!(
        t.output,
        vec![Output::Axis {
            code: "ABS_HAT0Y".into(),
            value: 1.0
        }]
    );
    let t = step(t.state, &p, button("BTN_DPAD_DOWN", false), 2);
    assert_eq!(
        t.output,
        vec![Output::Axis {
            code: "ABS_HAT0Y".into(),
            value: -1.0
        }]
    );
    assert_eq!(
        step(t.state, &p, button("BTN_DPAD_UP", false), 3).output,
        vec![Output::Axis {
            code: "ABS_HAT0Y".into(),
            value: 0.0
        }]
    );
}
#[test]
fn trigger_inversion_preserves_unsigned_range() {
    let mut p = Profile::default();
    p.axes.insert(
        "ABS_Z".into(),
        AxisBinding {
            invert: true,
            ..Default::default()
        },
    );
    let t = step(
        State::default(),
        &p,
        Input::Axis {
            code: "ABS_Z".into(),
            value: 0.2,
        },
        0,
    );
    assert_eq!(
        t.output,
        vec![Output::Axis {
            code: "ABS_Z".into(),
            value: 0.8
        }]
    );
}
#[test]
fn mouse_resumes_without_accumulating_idle_time() {
    let mut p = Profile::default();
    p.axes.insert(
        "ABS_X".into(),
        AxisBinding {
            mode: AxisMode::Mouse,
            target: Some("REL_X".into()),
            sensitivity: 1000.0,
            ..Default::default()
        },
    );
    let t = step(
        State::default(),
        &p,
        Input::Axis {
            code: "ABS_X".into(),
            value: 1.0,
        },
        0,
    );
    let t = step(t.state, &p, Input::Tick, 8000);
    assert_eq!(
        t.output,
        vec![Output::Relative {
            code: "REL_X".into(),
            value: 8
        }]
    );
    let t = step(
        t.state,
        &p,
        Input::Axis {
            code: "ABS_X".into(),
            value: 0.0,
        },
        9000,
    );
    assert_eq!(t.next_deadline, None);
    let t = step(
        t.state,
        &p,
        Input::Axis {
            code: "ABS_X".into(),
            value: 1.0,
        },
        1_000_000,
    );
    assert_eq!(t.next_deadline, Some(1_008_000));
    assert_eq!(
        step(t.state, &p, Input::Tick, 1_008_000).output,
        vec![Output::Relative {
            code: "REL_X".into(),
            value: 8
        }]
    );
}
