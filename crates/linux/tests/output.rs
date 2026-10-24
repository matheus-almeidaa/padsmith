use anyhow::{Context, Result};
use evdev::{InputEvent, raw_stream::RawDevice};
use padsmith_core::{Profile, State, step};
use padsmith_linux::{devices::normalize, output};
use padsmith_protocol::NodeInfo;
use std::{collections::BTreeMap, time::Duration};

#[tokio::test]
#[ignore = "requer uinput/uaccess; testa apenas saída virtual, sem iniciar daemon"]
async fn xbox360_face_buttons_through_uinput() -> Result<()> {
    // This output name is always excluded from physical discovery. No second
    // daemon is started and no physical controller is read or captured.
    let name = output::controller_name(&format!("output-regression-{}", std::process::id()));
    let mut device = output::gamepad(&name)?;
    let mut reader = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some((path, _)) = evdev::enumerate().find(|(_, d)| d.name() == Some(&name))
                && let Ok(reader) = RawDevice::open(path)
            {
                break reader;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .context("Saída virtual sem ACL de leitura")?;
    assert_eq!(reader.input_id().vendor(), 0x045e);
    assert_eq!(reader.input_id().product(), 0x028e);
    reader.grab()?;
    let mut stream = reader.into_event_stream()?;
    let node = NodeInfo {
        role: "gamepad".into(),
        primary: true,
        ..Default::default()
    };
    let swap: Profile = toml::from_str(include_str!("../../../examples/swap-xy.toml"))?;
    for (profile, cases) in [
        (
            Profile::default(),
            vec![(304, 304), (305, 305), (308, 307), (307, 308)],
        ),
        (swap, vec![(308, 308), (307, 307)]),
    ] {
        for (physical, xbox) in cases {
            let mut state = State::default();
            for value in [1, 0] {
                let input = normalize(&node, &BTreeMap::new(), InputEvent::new(1, physical, value))
                    .unwrap();
                let result = step(state, &profile, input, (1 - value) as u64);
                output::emit_gamepad(&mut device, &result.output)?;
                let event = tokio::time::timeout(Duration::from_secs(2), async {
                    loop {
                        let event = stream.next_event().await?;
                        if event.event_type().0 == 1 {
                            break Ok::<_, std::io::Error>(event);
                        }
                    }
                })
                .await??;
                assert_eq!((event.code(), event.value()), (xbox, value));
                state = result.state;
            }
        }
    }
    Ok(())
}
