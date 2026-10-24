//! Opt-in real kernel integration. Only the synthetic source is managed.
use anyhow::{Context, Result};
use evdev::{
    InputEvent,
    raw_stream::{EventStream, RawDevice},
};
use padsmith_core::{Binding, ButtonMode, Profile, Target};
use padsmith_protocol::{Command, Message, Request};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command as Process, Stdio},
    time::{Duration, Instant},
};
use tokio::{io::BufReader, net::UnixStream};

struct ChildGuard {
    child: Child,
    root: PathBuf,
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.root);
    }
}
async fn rpc(root: &Path, command: Command) -> Result<Message> {
    let mut connection = UnixStream::connect(root.join("padsmith/control.sock")).await?;
    padsmith_protocol::write_message(
        &mut connection,
        &Request {
            version: padsmith_protocol::VERSION,
            request: command,
        },
    )
    .await?;
    padsmith_protocol::read_message(&mut BufReader::new(connection)).await
}
async fn key(stream: &mut EventStream) -> Result<(u16, i32)> {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let event = stream.next_event().await?;
            if event.event_type().0 == 1 {
                return Ok((event.code(), event.value()));
            }
        }
    })
    .await?
}
async fn found(name: &str) -> Result<padsmith_protocol::DeviceInfo> {
    let start = Instant::now();
    loop {
        if let Some(d) = padsmith_linux::devices::discover()?
            .into_iter()
            .find(|d| d.name == name)
            && RawDevice::open(&d.nodes[0].path).is_ok()
        {
            return Ok(d);
        }
        anyhow::ensure!(
            start.elapsed() < Duration::from_secs(5),
            "Fonte sintética inacessível: {name}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
async fn status_when(
    root: &Path,
    predicate: impl Fn(&padsmith_protocol::Status) -> bool,
) -> Result<padsmith_protocol::Status> {
    let start = Instant::now();
    loop {
        if let Ok(Message::Status { status }) = rpc(root, Command::Status).await
            && predicate(&status)
        {
            return Ok(status);
        }
        anyhow::ensure!(
            start.elapsed() < Duration::from_secs(5),
            "Estado esperado do daemon não foi observado"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}
async fn output_reader(id: &str) -> Result<EventStream> {
    let start = Instant::now();
    let name = padsmith_linux::output::controller_name(id);
    loop {
        if let Some((path, _)) = evdev::enumerate().find(|(_, d)| d.name() == Some(&name)) {
            let mut d = RawDevice::open(path)?;
            d.grab()?;
            return Ok(d.into_event_stream()?);
        }
        anyhow::ensure!(
            start.elapsed() < Duration::from_secs(5),
            "Saída virtual inacessível: {id}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requer /dev/uinput e ACL uaccess; cria apenas controles sintéticos"]
async fn daemon_swap_toggle_pause_release_and_latency() -> Result<()> {
    let mut source = padsmith_linux::output::gamepad("Padsmith Test Source")?;
    let started = Instant::now();
    let device = loop {
        if let Some(device) = padsmith_linux::devices::discover()?
            .into_iter()
            .find(|d| d.name == "Padsmith Test Source")
            && RawDevice::open(&device.nodes[0].path).is_ok()
        {
            break device;
        }
        anyhow::ensure!(
            started.elapsed() < Duration::from_secs(5),
            "udev não concedeu acesso ao controle sintético"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    let root = std::env::temp_dir().join(format!("padsmith-integration-{}", std::process::id()));
    fs::create_dir_all(root.join("config/profiles"))?;
    let mut p = Profile::default();
    for (a, b) in [("BTN_WEST", "BTN_NORTH"), ("BTN_NORTH", "BTN_WEST")] {
        p.buttons.insert(
            a.into(),
            Binding {
                output: Some(Target::Gamepad { code: b.into() }),
                ..Default::default()
            },
        );
    }
    p.buttons.insert(
        "BTN_SOUTH".into(),
        Binding {
            output: Some(Target::Gamepad {
                code: "BTN_SOUTH".into(),
            }),
            mode: ButtonMode::Toggle,
            ..Default::default()
        },
    );
    p.macros.insert(
        "test".into(),
        padsmith_core::Macro {
            name: "Pulso sintético".into(),
            steps: vec![padsmith_core::MacroStep::Hold {
                target: Target::Gamepad {
                    code: "BTN_EAST".into(),
                },
                ms: 10,
            }],
        },
    );
    p.triggers.push(padsmith_core::MacroTrigger {
        id: "chord".into(),
        buttons: vec!["BTN_TL".into(), "BTN_TR".into()],
        macro_id: "test".into(),
        kind: padsmith_core::TriggerKind::Chord,
        ..Default::default()
    });
    padsmith_linux::profiles::save(&root.join("config/profiles"), &p)?;
    let child = Process::new(env!("CARGO_BIN_EXE_padsmithd"))
        .args([
            "--config",
            root.join("config").to_str().unwrap(),
            "--device",
            &device.id,
        ])
        .env("XDG_RUNTIME_DIR", &root)
        .env_remove("NOTIFY_SOCKET")
        .env_remove("WATCHDOG_USEC")
        .env("PADSMITH_NOTIFICATIONS", "0")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;
    let mut guard = ChildGuard {
        child,
        root: root.clone(),
    };
    let started = Instant::now();
    loop {
        if let Ok(Message::Status { status }) = rpc(&root, Command::Status).await
            && status
                .controllers
                .iter()
                .any(|c| c.device.id == device.id && c.error.is_none())
        {
            break;
        }
        if let Some(status) = guard.child.try_wait()? {
            anyhow::bail!("Daemon terminou: {status}");
        }
        anyhow::ensure!(
            started.elapsed() < Duration::from_secs(5),
            "Daemon não ficou pronto"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let started = Instant::now();
    let mut reader = loop {
        let name = padsmith_linux::output::controller_name(&device.id);
        let found = evdev::enumerate().find(|(_, d)| d.name() == Some(name.as_str()));
        if let Some((path, _)) = found {
            break RawDevice::open(path)?;
        }
        anyhow::ensure!(
            started.elapsed() < Duration::from_secs(5),
            "Saída virtual inacessível"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    assert_eq!(reader.input_id().vendor(), 0x045e);
    assert_eq!(reader.input_id().product(), 0x028e);
    reader.grab()?;
    let mut stream = reader.into_event_stream()?;
    // Raw source codes are positional. After the profile's X/Y swap, encode
    // the destination using xpad's ABI (Xbox X=307, Y=308).
    for (input, expected) in [(308, 308), (307, 307)] {
        for value in [1, 0] {
            source.emit(&[InputEvent::new(1, input, value)])?;
            assert_eq!(key(&mut stream).await?, (expected, value));
        }
    }
    source.emit(&[InputEvent::new(1, 304, 1), InputEvent::new(1, 304, 0)])?;
    assert_eq!(key(&mut stream).await?, (304, 1));
    assert!(
        tokio::time::timeout(Duration::from_millis(30), key(&mut stream))
            .await
            .is_err(),
        "Toggle não deve soltar ao liberar o botão físico"
    );
    let response = rpc(
        &root,
        Command::Pause {
            device: None,
            paused: true,
        },
    )
    .await?;
    assert!(matches!(response, Message::Ok));
    assert_eq!(key(&mut stream).await?, (304, 0));
    source.emit(&[InputEvent::new(1, 307, 1), InputEvent::new(1, 307, 0)])?;
    assert_eq!(key(&mut stream).await?, (308, 1));
    assert_eq!(key(&mut stream).await?, (308, 0));
    rpc(
        &root,
        Command::Pause {
            device: None,
            paused: false,
        },
    )
    .await?;
    let mut latency = Vec::new();
    let hotplug = tokio::task::spawn_blocking(|| -> Result<()> {
        std::thread::sleep(Duration::from_millis(15));
        let source = padsmith_linux::output::gamepad("Padsmith Hotplug Latency")?;
        std::thread::sleep(Duration::from_millis(150));
        drop(source);
        Ok(())
    });
    for _ in 0..256 {
        let start = Instant::now();
        source.emit(&[InputEvent::new(1, 307, 1)])?;
        assert_eq!(key(&mut stream).await?, (307, 1));
        latency.push(start.elapsed().as_micros());
        source.emit(&[InputEvent::new(1, 307, 0)])?;
        assert_eq!(key(&mut stream).await?, (307, 0));
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    hotplug.await??;
    latency.sort_unstable();
    println!(
        "latência uinput→daemon→uinput: p50={} µs p95={} µs p99={} µs max={} µs",
        latency[128], latency[243], latency[253], latency[255]
    );
    // The same daemon timer drives a real uinput macro, not just the pure tests.
    source.emit(&[InputEvent::new(1, 310, 1), InputEvent::new(1, 311, 1)])?;
    assert_eq!(key(&mut stream).await?, (305, 1));
    assert_eq!(key(&mut stream).await?, (305, 0));
    source.emit(&[InputEvent::new(1, 310, 0), InputEvent::new(1, 311, 0)])?;
    let response = rpc(
        &root,
        Command::RecordStart {
            device: device.id.clone(),
        },
    )
    .await?;
    assert!(matches!(response, Message::Ok));
    source.emit(&[InputEvent::new(1, 307, 1)])?;
    tokio::time::sleep(Duration::from_millis(10)).await;
    source.emit(&[InputEvent::new(1, 307, 0)])?;
    tokio::time::sleep(Duration::from_millis(5)).await;
    assert!(
        tokio::time::timeout(Duration::from_millis(10), key(&mut stream))
            .await
            .is_err(),
        "Gravação deve neutralizar as saídas"
    );
    let response = rpc(
        &root,
        Command::RecordStop {
            device: device.id.clone(),
        },
    )
    .await?;
    let Message::Recording { recording } = response else {
        anyhow::bail!("Resposta de gravação ausente");
    };
    assert!(recording.sequence.steps.iter().any(|s|matches!(s,padsmith_core::MacroStep::Press {target:Target::Gamepad {code}} if code=="BTN_WEST")),"{recording:?}");
    rpc(
        &root,
        Command::RunMacro {
            device: device.id.clone(),
            macro_id: "test".into(),
        },
    )
    .await?;
    assert_eq!(key(&mut stream).await?, (305, 1));
    assert_eq!(key(&mut stream).await?, (305, 0));
    source.emit(&[InputEvent::new(1, 307, 1)])?;
    assert_eq!(key(&mut stream).await?, (307, 1));
    // SAFETY: signal is sent only to the child process owned by this test.
    unsafe { libc::kill(guard.child.id() as i32, libc::SIGTERM) };
    assert_eq!(
        key(&mut stream)
            .await
            .context("Saída deve ser solta antes de remover o dispositivo")?,
        (307, 0)
    );
    let status = guard.child.wait()?;
    assert!(status.success());
    let mut physical = RawDevice::open(&device.nodes[0].path)?;
    physical.grab()?;
    physical.ungrab()?;
    Ok(())
}

struct ProcessGuard(Child);
impl Drop for ProcessGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requer uinput/uaccess; cria dois controles e um processo sintéticos"]
async fn multi_controller_hotplug_auto_profiles_emergency_and_crash() -> Result<()> {
    let mut first = padsmith_linux::output::gamepad("Padsmith Multi First")?;
    let mut second = padsmith_linux::output::gamepad("Padsmith Multi Second")?;
    let a = found("Padsmith Multi First").await?;
    let b = found("Padsmith Multi Second").await?;
    let root = std::env::temp_dir().join(format!("padsmith-multi-{}", std::process::id()));
    let profiles = root.join("config/profiles");
    fs::create_dir_all(&profiles)?;
    let mut base = Profile::default();
    base.emergency.hold_ms = 50;
    let mut alternate = base.clone();
    alternate.id = "second".into();
    alternate.name = "Segundo controle".into();
    alternate.buttons.insert(
        "BTN_SOUTH".into(),
        Binding {
            mode: ButtonMode::Toggle,
            output: Some(Target::Gamepad {
                code: "BTN_EAST".into(),
            }),
            ..Default::default()
        },
    );
    let mut game = base.clone();
    game.id = "game".into();
    game.name = "Processo sintético".into();
    game.processes = vec!["padsmith-integration-game.exe".into()];
    game.devices = vec![a.id.clone()];
    game.buttons.insert(
        "BTN_NORTH".into(),
        Binding {
            output: Some(Target::Gamepad {
                code: "BTN_EAST".into(),
            }),
            ..Default::default()
        },
    );
    for p in [&base, &alternate, &game] {
        padsmith_linux::profiles::save(&profiles, p)?;
    }
    let child = Process::new(env!("CARGO_BIN_EXE_padsmithd"))
        .args([
            "--config",
            root.join("config").to_str().unwrap(),
            "--device",
            "name:Padsmith Multi First",
            "--device",
            "name:Padsmith Multi Second",
        ])
        .env("XDG_RUNTIME_DIR", &root)
        .env("PADSMITH_NOTIFICATIONS", "0")
        .env_remove("NOTIFY_SOCKET")
        .env_remove("WATCHDOG_USEC")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;
    let mut daemon = ChildGuard {
        child,
        root: root.clone(),
    };
    status_when(&root, |s| {
        s.controllers.iter().filter(|c| c.error.is_none()).count() == 2
    })
    .await?;
    let mut out_a = output_reader(&a.id).await?;
    let mut out_b = output_reader(&b.id).await?;
    rpc(
        &root,
        Command::Apply {
            device: Some(b.id.clone()),
            profile: "second".into(),
        },
    )
    .await?;
    second.emit(&[InputEvent::new(1, 304, 1), InputEvent::new(1, 304, 0)])?;
    assert_eq!(key(&mut out_b).await?, (305, 1));
    first.emit(&[InputEvent::new(1, 304, 1)])?;
    assert_eq!(key(&mut out_a).await?, (304, 1));
    drop(second);
    assert_eq!(key(&mut out_b).await?, (305, 0));
    status_when(&root, |s| s.controllers.iter().all(|c| c.device.id != b.id)).await?;
    let mut second = padsmith_linux::output::gamepad("Padsmith Multi Second")?;
    let reconnected = status_when(&root, |s| {
        s.controllers.iter().any(|c| {
            c.device.name == "Padsmith Multi Second" && c.device.id != b.id && c.error.is_none()
        })
    })
    .await?;
    let new_b = &reconnected
        .controllers
        .iter()
        .find(|c| c.device.name == "Padsmith Multi Second")
        .unwrap()
        .device;
    let mut out_b = output_reader(&new_b.id).await?;
    assert!(
        tokio::time::timeout(Duration::from_millis(20), key(&mut out_a))
            .await
            .is_err(),
        "Reconectar outro controle não pode soltar o primeiro"
    );
    first.emit(&[InputEvent::new(1, 304, 0)])?;
    assert_eq!(key(&mut out_a).await?, (304, 0));
    // Default profile: preserve all four physical positions while matching
    // the Xbox 360 wire ABI, independently of a configured X/Y remap.
    for (physical, xbox) in [(304, 304), (305, 305), (308, 307), (307, 308)] {
        for value in [1, 0] {
            second.emit(&[InputEvent::new(1, physical, value)])?;
            assert_eq!(key(&mut out_b).await?, (xbox, value));
        }
    }
    let executable = root.join("padsmith-integration-game.exe");
    let compiled = Process::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .args([
            "--edition=2024",
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/process.rs"),
            "-o",
        ])
        .arg(&executable)
        .output()
        .context("rustc é necessário para compilar o processo sintético")?;
    anyhow::ensure!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let process = Process::new(executable).spawn()?;
    let mut process = ProcessGuard(process);
    status_when(&root, |s| {
        s.controllers
            .iter()
            .any(|c| c.device.id == a.id && c.profile == "game" && c.automatic)
    })
    .await?;
    first.emit(&[InputEvent::new(1, 307, 1), InputEvent::new(1, 307, 0)])?;
    assert_eq!(key(&mut out_a).await?, (305, 1));
    assert_eq!(key(&mut out_a).await?, (305, 0));
    process.0.kill()?;
    process.0.wait()?;
    status_when(&root, |s| {
        s.controllers
            .iter()
            .any(|c| c.device.id == a.id && c.profile == "default" && !c.automatic)
    })
    .await?;
    for expected_pause in [true, false] {
        first.emit(&[InputEvent::new(1, 316, 1), InputEvent::new(1, 314, 1)])?;
        status_when(&root, |s| {
            s.controllers
                .iter()
                .any(|c| c.device.id == a.id && c.paused == expected_pause)
        })
        .await?;
        let mut edges = Vec::new();
        for _ in 0..4 {
            edges.push(key(&mut out_a).await?);
        }
        assert!(
            edges.contains(&(316, 1))
                && edges.contains(&(316, 0))
                && edges.contains(&(314, 1))
                && edges.contains(&(314, 0))
        );
        first.emit(&[InputEvent::new(1, 316, 0), InputEvent::new(1, 314, 0)])?;
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    first.emit(&[InputEvent::new(1, 304, 1)])?;
    assert_eq!(key(&mut out_a).await?, (304, 1));
    daemon.child.kill()?;
    daemon.child.wait()?;
    let mut physical = RawDevice::open(&a.nodes[0].path)?;
    physical.grab()?;
    physical.ungrab()?;
    println!(
        "dois controles, perfis independentes, hotplug, .exe sintético, emergência e liberação após SIGKILL: ok"
    );
    Ok(())
}
