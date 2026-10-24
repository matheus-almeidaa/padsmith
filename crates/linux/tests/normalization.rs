use evdev::InputEvent;
use padsmith_core::Input;
use padsmith_protocol::NodeInfo;
use std::collections::BTreeMap;
#[test]
fn motion_axes_are_signed_and_keep_their_node_identity() {
    let node = NodeInfo {
        role: "motion".into(),
        ..Default::default()
    };
    let input = padsmith_linux::devices::normalize(
        &node,
        &BTreeMap::from([(2, (-32768, 32767))]),
        InputEvent::new(3, 2, 0),
    );
    assert!(
        matches!(input,Some(Input::Axis {code,value}) if code=="motion:ABS_Z"&&value.abs()<0.001)
    );
}
#[test]
fn every_valid_evdev_key_code_has_a_round_trip_name() {
    for code in 0..=0x2ff {
        assert_eq!(
            padsmith_linux::key_code(&padsmith_linux::key_name(code)),
            Some(code)
        );
    }
}
#[test]
fn bundled_profiles_validate() {
    for text in [
        include_str!("../../../examples/swap-xy.toml"),
        include_str!("../../../examples/desktop.toml"),
        include_str!("../../../examples/macros.toml"),
        include_str!("../../../examples/game.toml"),
    ] {
        let profile = toml::from_str(text).unwrap();
        padsmith_linux::validate_profile(&profile).unwrap();
    }
}
