use anyhow::{Context, Result, bail};
use clap::Parser;
use evdev::uinput::VirtualDevice;
use notify::Watcher;
use padsmith_core::{Input, Profile, State};
use padsmith_linux::{
    devices,
    output::{self, Desktop},
    profiles,
};
use padsmith_protocol::{Command, ControllerStatus, DeviceInfo, Message, Request, Status};
use std::{
    collections::BTreeMap,
    fs,
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, PermissionsExt},
    },
    path::PathBuf,
    time::{Duration, Instant},
};
use tokio::{
    io::BufReader,
    net::UnixListener,
    sync::{broadcast, mpsc, oneshot},
    task::JoinHandle,
};

#[derive(Parser)]
#[command(version, about = "Daemon de remapeamento evdev/uinput do Padsmith")]
struct Args {
    #[arg(long)]
    config: Option<PathBuf>,
    /// IDs físicos permitidos (repita a opção); name: aceita um nome exato para testes uinput.
    #[arg(long)]
    device: Vec<String>,
}
struct Rpc {
    command: Command,
    reply: oneshot::Sender<Message>,
}
enum Incoming {
    Input {
        id: String,
        generation: u64,
        input: Input,
    },
    Gone {
        id: String,
        generation: u64,
    },
    Reload,
    Discovered(std::result::Result<Vec<DeviceInfo>, String>),
    Processes(Vec<String>),
    Fatal(String),
    Prepared {
        info: Box<DeviceInfo>,
        token: u64,
        result: std::result::Result<(VirtualDevice, Vec<devices::CapturedNode>), String>,
    },
}
struct Session {
    info: DeviceInfo,
    profile: String,
    state: State,
    deadline: Option<u64>,
    gamepad: VirtualDevice,
    readers: Vec<JoinHandle<()>>,
    generation: u64,
    recording: Option<padsmith_core::Recorder>,
    controls: Vec<std::os::fd::OwnedFd>,
    manual_profile: String,
    auto_match: Option<String>,
}
struct Daemon {
    sessions: BTreeMap<String, Session>,
    errors: BTreeMap<String, ControllerStatus>,
    profiles: BTreeMap<String, std::sync::Arc<Profile>>,
    desktop: Desktop,
    directory: PathBuf,
    assignments: BTreeMap<String, String>,
    default_profile: String,
    events: broadcast::Sender<Message>,
    input: mpsc::Sender<Incoming>,
    epoch: Instant,
    generation: u64,
    desired: BTreeMap<String, DeviceInfo>,
    preparing: BTreeMap<String, u64>,
    preparing_since: BTreeMap<String, Instant>,
    processes: Vec<String>,
}
impl Daemon {
    fn now(&self) -> u64 {
        self.epoch.elapsed().as_micros() as u64
    }
    fn status(&self) -> Status {
        Status {
            controllers: self
                .sessions
                .values()
                .map(|s| ControllerStatus {
                    device: s.info.clone(),
                    profile: s.profile.clone(),
                    paused: s.state.paused,
                    error: None,
                    recording: s.recording.is_some(),
                    automatic: s.auto_match.as_ref() == Some(&s.profile),
                    layers: s.state.active_layers().to_vec(),
                })
                .chain(self.errors.values().cloned())
                .collect(),
            profiles: self.profiles.values().cloned().collect(),
            version: env!("CARGO_PKG_VERSION").into(),
            processes: self.processes.clone(),
        }
    }
    fn publish(&self) {
        let _ = self.events.send(Message::Status {
            status: self.status(),
        });
    }
    fn notice(&self, message: impl Into<String>) {
        let message = message.into();
        tracing::warn!(%message);
        let _ = self.events.send(Message::Notice { message });
    }
    fn process(&mut self, id: &str, input: Input) -> Result<()> {
        let now = self.now();
        let Some(s) = self.sessions.get_mut(id) else {
            return Ok(());
        };
        let Some(profile) = self.profiles.get(&s.profile) else {
            bail!("Perfil ativo ausente")
        };
        let was_recording = s.recording.is_some();
        if let Some(recording) = s.recording.as_mut() {
            recording.push(profile, &input, now);
        }
        let transition = padsmith_core::step(std::mem::take(&mut s.state), profile, input, now);
        s.state = transition.state;
        s.deadline = transition.next_deadline;
        if !was_recording {
            output::emit_gamepad(&mut s.gamepad, &transition.output)?;
            self.desktop.emit(id, &transition.output)?;
        }
        for effect in transition.effects {
            match effect {
                padsmith_core::Effect::PauseChanged { paused } => {
                    if let Some(session) = self.sessions.get_mut(id) {
                        session.recording = None;
                    }
                    self.announce(if paused {
                        "Remapeamento pausado"
                    } else {
                        "Remapeamento retomado"
                    });
                    self.publish();
                }
                padsmith_core::Effect::SelectProfile { profile } => {
                    if was_recording {
                        continue;
                    }
                    let profile = if profile == "$next" {
                        let ids: Vec<_> = self.profiles.keys().cloned().collect();
                        let current = self
                            .sessions
                            .get(id)
                            .map(|s| s.profile.as_str())
                            .unwrap_or("");
                        ids[(ids.iter().position(|p| p == current).unwrap_or(0) + 1) % ids.len()]
                            .clone()
                    } else {
                        profile
                    };
                    if let Err(e) = self.rpc(Command::Apply {
                        device: Some(id.into()),
                        profile,
                    }) {
                        self.notice(format!("Atalho não aplicado: {e:#}"));
                    }
                }
                padsmith_core::Effect::LayersChanged => self.publish(),
            }
        }
        Ok(())
    }
    fn stop(&mut self, id: &str) {
        if let Err(error) = self.process(id, Input::Reset) {
            tracing::error!(%error,"Falha ao neutralizar controle");
        }
        if let Err(error) = self.desktop.release(id) {
            tracing::error!(%error,"Falha ao liberar teclado/mouse");
        }
        if let Some(s) = self.sessions.remove(id) {
            for control in &s.controls {
                let _ = devices::ungrab(control);
            }
            for reader in s.readers {
                reader.abort();
            }
            tokio::task::spawn_blocking(move || drop(s.gamepad));
        }
    }
    fn attach(
        &mut self,
        info: DeviceInfo,
        gamepad: VirtualDevice,
        nodes: Vec<devices::CapturedNode>,
        generation: u64,
    ) {
        let profile = self
            .assignments
            .get(&format!("connection:{}", info.connection))
            .or_else(|| self.assignments.get(&info.id))
            .filter(|p| self.profiles.contains_key(*p))
            .cloned()
            .unwrap_or_else(|| self.default_profile.clone());
        let manual_profile = profile.clone();
        let auto_match =
            padsmith_linux::processes::choose(&self.profiles, &self.processes, &info.id);
        let profile = auto_match.clone().unwrap_or(profile);
        let mut readers = Vec::new();
        let mut controls = Vec::new();
        for mut node in nodes {
            controls.push(node.control);
            let sender = self.input.clone();
            let events = self.events.clone();
            let id = info.id.clone();
            let epoch = self.epoch;
            readers.push(tokio::spawn(async move {
                let mut dropped = false;
                loop {
                    match node.stream.next_event().await {
                        Ok(event) => {
                            if events.receiver_count() > 0 {
                                let message = Message::Raw {
                                    device: id.clone(),
                                    node: node.info.path.clone(),
                                    role: node.info.role.clone(),
                                    time_us: epoch.elapsed().as_micros() as u64,
                                    event_type: event.event_type().0,
                                    code: event.code(),
                                    value: event.value(),
                                    label: devices::event_label(event),
                                };
                                // Raw diagnostics never wait behind a slow subscriber.
                                let _ = events.send(message);
                            }
                            if event.event_type().0 == 0 && event.code() == 3 {
                                dropped = true;
                                if sender
                                    .send(Incoming::Input {
                                        id: id.clone(),
                                        generation,
                                        input: Input::Reset,
                                    })
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                                let _ = events.send(Message::Notice {
                                    message: format!(
                                        "SYN_DROPPED em {}; saídas neutralizadas até nova leitura",
                                        node.info.path
                                    ),
                                });
                                continue;
                            }
                            if dropped {
                                if event.event_type().0 == 0 && event.code() == 0 {
                                    // Query current absolute state after overflow; don't replay key edges as toggles.
                                    if node.info.primary
                                        && let Ok(state) = node.stream.device().get_abs_state()
                                    {
                                        for (code, abs) in state.iter().enumerate() {
                                            if !node.axes.contains_key(&(code as u16)) {
                                                continue;
                                            }
                                            let event =
                                                evdev::InputEvent::new(3, code as u16, abs.value);
                                            if let Some(input) =
                                                devices::normalize(&node.info, &node.axes, event)
                                            {
                                                let _ = sender
                                                    .send(Incoming::Input {
                                                        id: id.clone(),
                                                        generation,
                                                        input,
                                                    })
                                                    .await;
                                            }
                                        }
                                    }
                                    dropped = false;
                                }
                                continue;
                            }
                            if let Some(input) = devices::normalize(&node.info, &node.axes, event)
                                && sender
                                    .send(Incoming::Input {
                                        id: id.clone(),
                                        generation,
                                        input,
                                    })
                                    .await
                                    .is_err()
                            {
                                break;
                            }
                        }
                        Err(error) => {
                            tracing::info!(%error,node=%node.info.path,"Nó desconectado");
                            break;
                        }
                    }
                }
                let _ = sender.send(Incoming::Gone { id, generation }).await;
            }));
        }
        tracing::info!(device=%info.name,id=%info.id,%profile,"Controle capturado");
        self.errors.remove(&info.id);
        self.sessions.insert(
            info.id.clone(),
            Session {
                info,
                profile,
                state: State::default(),
                deadline: None,
                gamepad,
                readers,
                generation,
                recording: None,
                controls,
                manual_profile,
                auto_match,
            },
        );
    }
    fn reconcile(&mut self, found: Vec<DeviceInfo>, filter: &[String]) {
        let found: BTreeMap<_, _> = found
            .into_iter()
            .filter(|d| {
                (filter.is_empty()
                    && ![
                        "Padsmith Test Source",
                        "Padsmith Multi First",
                        "Padsmith Multi Second",
                        "Padsmith Hotplug Latency",
                    ]
                    .contains(&d.name.as_str()))
                    || filter.iter().any(|id| {
                        id.strip_prefix("name:")
                            .map(|name| name == d.name)
                            .unwrap_or(d.id == *id)
                    })
            })
            .map(|d| (d.id.clone(), d))
            .collect();
        let removed: Vec<_> = self
            .sessions
            .iter()
            .filter(|(id, s)| found.get(*id).is_none_or(|d| d.nodes != s.info.nodes))
            .map(|(id, _)| id.clone())
            .collect();
        for id in removed {
            self.stop(&id);
        }
        self.errors.retain(|id, _| found.contains_key(id));
        self.desired = found.clone();
        for (id, info) in found {
            if let Some(s) = self.sessions.get_mut(&id) {
                s.info = info;
                continue;
            }
            if self.preparing.contains_key(&id) {
                continue;
            }
            self.generation += 1;
            let token = self.generation;
            self.preparing.insert(id, token);
            self.preparing_since.insert(info.id.clone(), Instant::now());
            let sender = self.input.clone();
            // Device setup may wait for kernel/udev. Never stall active controllers.
            tokio::task::spawn_blocking(move || {
                let result = (|| -> Result<_> {
                    let nodes = devices::capture(&info)
                        .context("Falha ao capturar todos os nós; grabs parciais liberados")?;
                    let gamepad = output::gamepad(&output::controller_name(&info.id))?;
                    Ok((gamepad, nodes))
                })()
                .map_err(|e| format!("{e:#}"));
                let _ = sender.blocking_send(Incoming::Prepared {
                    info: Box::new(info),
                    token,
                    result,
                });
            });
        }
        self.publish();
    }
    fn save_assignments(&self) -> Result<()> {
        profiles::atomic_write(
            &self.directory.join("assignments.toml"),
            toml::to_string(&self.assignments)?.as_bytes(),
        )
    }
    fn announce(&self, message: impl Into<String>) {
        let message = message.into();
        tracing::info!(%message);
        let _ = self.events.send(Message::Notice {
            message: message.clone(),
        });
        if std::env::var("PADSMITH_NOTIFICATIONS").as_deref() == Ok("0") {
            return;
        }
        tokio::spawn(async move {
            match tokio::process::Command::new("notify-send")
                .args([
                    "--app-name=Padsmith",
                    "--icon=io.padsmith.desktop",
                    "--expire-time=2500",
                    "--",
                    "Padsmith",
                    &message,
                ])
                .status()
                .await
            {
                Ok(status) if status.success() => {}
                result => tracing::debug!(?result, "Notificação indisponível"),
            }
        });
    }
    fn automatic(&mut self) -> Result<()> {
        let updates: Vec<_> = self
            .sessions
            .iter()
            .filter_map(|(id, s)| {
                let found = padsmith_linux::processes::choose(&self.profiles, &self.processes, id);
                (found != s.auto_match).then(|| {
                    (
                        id.clone(),
                        found.clone().unwrap_or_else(|| s.manual_profile.clone()),
                        found,
                    )
                })
            })
            .collect();
        for (id, profile, auto_match) in updates {
            self.process(&id, Input::Reset)?;
            if let Some(s) = self.sessions.get_mut(&id) {
                s.profile = profile.clone();
                s.auto_match = auto_match;
                s.recording = None;
            }
            if let Some(p) = self.profiles.get(&profile) {
                self.announce(format!("Perfil: {}", p.name));
            }
        }
        self.publish();
        Ok(())
    }
    fn rpc(&mut self, command: Command) -> Result<Message> {
        match command {
            Command::RecordStart { device } => {
                let s = self
                    .sessions
                    .get(&device)
                    .context("Controle não conectado")?;
                anyhow::ensure!(
                    s.recording.is_none(),
                    "Já existe uma gravação neste controle"
                );
                self.process(&device, Input::Reset)?;
                let recorder = padsmith_core::Recorder::new(self.now());
                self.sessions.get_mut(&device).unwrap().recording = Some(recorder);
            }
            Command::RecordStop { device } => {
                let recorder = self
                    .sessions
                    .get_mut(&device)
                    .context("Controle não conectado")?
                    .recording
                    .take()
                    .context("Nenhuma gravação ativa")?;
                let recording = recorder.finish(self.now());
                self.process(&device, Input::Reset)?;
                self.publish();
                return Ok(Message::Recording { recording });
            }
            Command::RunMacro { device, macro_id } => {
                let s = self
                    .sessions
                    .get(&device)
                    .context("Controle não conectado")?;
                anyhow::ensure!(
                    !s.state.paused && s.recording.is_none(),
                    "Retome o remapeamento e encerre a gravação antes de executar"
                );
                anyhow::ensure!(
                    self.profiles[&s.profile].macros.contains_key(&macro_id),
                    "Macro não encontrada no perfil ativo"
                );
                self.process(&device, Input::Macro { macro_id })?;
            }
            Command::Status | Command::Watch { .. } => {
                return Ok(Message::Status {
                    status: self.status(),
                });
            }
            Command::SaveProfile { profile } => {
                let profile = *profile;
                padsmith_linux::validate_profile(&profile)?;
                profiles::save(&self.directory.join("profiles"), &profile)?;
                let ids: Vec<_> = self
                    .sessions
                    .iter()
                    .filter(|(_, s)| s.profile == profile.id)
                    .map(|(id, _)| id.clone())
                    .collect();
                for id in ids {
                    self.process(&id, Input::Reset)?;
                }
                self.profiles
                    .insert(profile.id.clone(), std::sync::Arc::new(profile));
                self.automatic()?;
            }
            Command::Apply { device, profile } => {
                anyhow::ensure!(
                    self.profiles.contains_key(&profile),
                    "Perfil não encontrado: {profile}"
                );
                let profile_name = self.profiles[&profile].name.clone();
                let ids: Vec<_> = self
                    .sessions
                    .keys()
                    .filter(|id| device.as_ref().is_none_or(|d| d == *id))
                    .cloned()
                    .collect();
                if let Some(ref device) = device {
                    anyhow::ensure!(!ids.is_empty(), "Controle não conectado: {device}");
                }
                for id in ids {
                    self.process(&id, Input::Reset)?;
                    self.sessions.get_mut(&id).unwrap().profile = profile.clone();
                    let s = self.sessions.get_mut(&id).unwrap();
                    s.manual_profile = profile.clone();
                    s.recording = None;
                    self.assignments
                        .insert(format!("connection:{}", s.info.connection), profile.clone());
                    self.assignments.insert(id, profile.clone());
                }
                if device.is_none() {
                    self.default_profile = profile.clone();
                    self.assignments.insert("default".into(), profile);
                }
                self.save_assignments()?;
                self.announce(format!("Perfil: {profile_name}"));
            }
            Command::Pause { device, paused } => {
                let ids: Vec<_> = self
                    .sessions
                    .keys()
                    .filter(|id| device.as_ref().is_none_or(|d| d == *id))
                    .cloned()
                    .collect();
                if device.is_some() {
                    anyhow::ensure!(!ids.is_empty(), "Controle não conectado");
                }
                for id in ids {
                    self.process(&id, Input::Reset)?;
                    self.sessions.get_mut(&id).unwrap().state.paused = paused;
                    self.sessions.get_mut(&id).unwrap().recording = None;
                }
            }
        }
        self.publish();
        Ok(Message::Ok)
    }
    fn reload(&mut self) -> Result<()> {
        let new = profiles::load(&self.directory.join("profiles"))?
            .into_iter()
            .map(|(id, p)| (id, std::sync::Arc::new(p)))
            .collect();
        if new == self.profiles {
            return Ok(());
        }
        let ids: Vec<_> = self.sessions.keys().cloned().collect();
        for id in &ids {
            self.process(id, Input::Reset)?;
        }
        self.profiles = new;
        if !self.profiles.contains_key(&self.default_profile) {
            self.default_profile = self
                .profiles
                .keys()
                .next()
                .context("Nenhum perfil")?
                .clone();
        }
        for s in self.sessions.values_mut() {
            if !self.profiles.contains_key(&s.profile) {
                s.profile = self.default_profile.clone();
            }
            if !self.profiles.contains_key(&s.manual_profile) {
                s.manual_profile = self.default_profile.clone();
            }
        }
        self.automatic()?;
        self.publish();
        Ok(())
    }
}

fn runtime_socket() -> Result<(UnixListener, fs::File, PathBuf)> {
    let path = padsmith_protocol::socket_path()?;
    let dir = path.parent().unwrap();
    fs::create_dir_all(dir)?;
    let meta = fs::symlink_metadata(dir)?;
    // SAFETY: getuid has no preconditions.
    anyhow::ensure!(
        meta.is_dir() && meta.uid() == unsafe { libc::getuid() },
        "Diretório de runtime não pertence ao usuário"
    );
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("daemon.lock"))?;
    // SAFETY: lock is a live descriptor; flock does not borrow memory.
    anyhow::ensure!(
        unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0,
        "Outro padsmithd já está em execução"
    );
    if path.exists() {
        fs::remove_file(&path)?;
    }
    let listener = UnixListener::bind(&path)?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    Ok((listener, lock, path))
}
async fn serve(
    listener: UnixListener,
    sender: mpsc::Sender<Rpc>,
    events: broadcast::Sender<Message>,
) -> Result<()> {
    loop {
        let (stream, _) = listener.accept().await?;
        // SAFETY: getuid has no preconditions.
        if stream.peer_cred()?.uid() != unsafe { libc::getuid() } {
            continue;
        }
        let sender = sender.clone();
        let events = events.clone();
        tokio::spawn(async move {
            let result = async {
                let mut reader = BufReader::new(stream);
                let request: Request = tokio::time::timeout(
                    Duration::from_secs(5),
                    padsmith_protocol::read_message(&mut reader),
                )
                .await??;
                if request.version != padsmith_protocol::VERSION {
                    padsmith_protocol::write_message(
                        reader.get_mut(),
                        &Message::Error {
                            message: "Versão IPC incompatível".into(),
                        },
                    )
                    .await?;
                    return Ok::<(), anyhow::Error>(());
                }
                let watch = if let Command::Watch { device } = &request.request {
                    Some(device.clone())
                } else {
                    None
                };
                let mut subscription = events.subscribe();
                let (reply, receive) = oneshot::channel();
                sender
                    .send(Rpc {
                        command: request.request,
                        reply,
                    })
                    .await?;
                let message = receive.await?;
                padsmith_protocol::write_message(reader.get_mut(), &message).await?;
                if let Some(device) = watch {
                    loop {
                        let message = match subscription.recv().await {
                            Ok(Message::Raw { device: ref id, .. })
                                if device.as_ref().is_some_and(|d| d != id) =>
                            {
                                continue;
                            }
                            Ok(message) => message,
                            Err(broadcast::error::RecvError::Lagged(count)) => {
                                Message::Dropped { count }
                            }
                            Err(_) => break,
                        };
                        padsmith_protocol::write_message(reader.get_mut(), &message).await?;
                    }
                }
                Ok(())
            }
            .await;
            if let Err(error) = result {
                tracing::debug!(%error,"Cliente IPC encerrado");
            }
        });
    }
}
fn systemd_notify(message: &str) {
    use std::os::{
        linux::net::SocketAddrExt,
        unix::net::{SocketAddr, UnixDatagram},
    };
    let Some(path) = std::env::var_os("NOTIFY_SOCKET") else {
        return;
    };
    let result = (|| -> std::io::Result<()> {
        let socket = UnixDatagram::unbound()?;
        let path = path.to_string_lossy();
        let address = if let Some(name) = path.strip_prefix('@') {
            SocketAddr::from_abstract_name(name.as_bytes())?
        } else {
            SocketAddr::from_pathname(path.as_ref())?
        };
        socket.send_to_addr(message.as_bytes(), &address)?;
        Ok(())
    })();
    if let Err(error) = result {
        tracing::debug!(%error,"sd_notify indisponível");
    }
}
async fn until(deadline: Option<Instant>) {
    match deadline {
        Some(d) => tokio::time::sleep_until(d.into()).await,
        None => std::future::pending().await,
    }
}
fn scan(sender: mpsc::Sender<Incoming>) {
    tokio::task::spawn_blocking(move || {
        let result = devices::discover().map_err(|e| format!("{e:#}"));
        let _ = sender.blocking_send(Incoming::Discovered(result));
    });
}

// The input actor is polled on the main thread. IPC encoding, input readers and
// timers use separate workers; large profile snapshots only clone Arc handles.
#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "padsmithd=info,padsmith_linux=info".into()),
        )
        .init();
    let args = Args::parse();
    let directory = args.config.unwrap_or(padsmith_protocol::config_dir()?);
    let loaded: BTreeMap<_, _> = profiles::load(&directory.join("profiles"))?
        .into_iter()
        .map(|(id, p)| (id, std::sync::Arc::new(p)))
        .collect();
    let assignments: BTreeMap<String, String> =
        match fs::read_to_string(directory.join("assignments.toml")) {
            Ok(s) => toml::from_str(&s)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(e) => return Err(e.into()),
        };
    let default_profile = assignments
        .get("default")
        .filter(|p| loaded.contains_key(*p))
        .cloned()
        .unwrap_or_else(|| {
            if loaded.contains_key("default") {
                "default".into()
            } else {
                loaded.keys().next().unwrap().clone()
            }
        });
    let (listener, _lock, socket) = runtime_socket()?;
    let (events, _) = broadcast::channel(4096);
    let (input, mut inputs) = mpsc::channel(2048);
    let (rpc, mut rpcs) = mpsc::channel(32);
    let (hotplug, mut changes) = mpsc::channel(1);
    devices::hotplug(hotplug);
    let rpc_events = events.clone();
    let server_input = input.clone();
    tokio::spawn(async move {
        if let Err(error) = serve(listener, rpc, rpc_events).await {
            tracing::error!(%error,"Servidor IPC falhou");
            let _ = server_input
                .send(Incoming::Fatal(format!("Servidor IPC: {error:#}")))
                .await;
        }
    });
    let watch_input = input.clone();
    let mut watcher =
        notify::recommended_watcher(move |result: Result<notify::Event, notify::Error>| {
            if result.is_ok_and(|e| {
                e.paths
                    .iter()
                    .any(|p| p.extension().is_some_and(|s| s == "toml"))
            }) {
                let _ = watch_input.try_send(Incoming::Reload);
            }
        })?;
    watcher.watch(
        &directory.join("profiles"),
        notify::RecursiveMode::NonRecursive,
    )?;
    let epoch = Instant::now();
    let mut daemon = Daemon {
        sessions: BTreeMap::new(),
        errors: BTreeMap::new(),
        profiles: loaded,
        desktop: Desktop::new().context("Sem acesso a /dev/uinput; execute padsmith doctor")?,
        directory,
        assignments,
        default_profile,
        events,
        input,
        epoch,
        generation: 0,
        desired: BTreeMap::new(),
        preparing: BTreeMap::new(),
        preparing_since: BTreeMap::new(),
        processes: Vec::new(),
    };
    scan(daemon.input.clone());
    let mut discovering = true;
    let mut scan_again = false;
    let processes_input = daemon.input.clone();
    tokio::spawn(async move {
        let mut previous = Vec::new();
        loop {
            if let Ok(Ok(names)) =
                tokio::task::spawn_blocking(padsmith_linux::processes::snapshot).await
                && names != previous
            {
                previous = names.clone();
                if processes_input
                    .send(Incoming::Processes(names))
                    .await
                    .is_err()
                {
                    break;
                }
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });
    systemd_notify("READY=1\nSTATUS=Padsmith ativo");
    let mut signal = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let watchdog = std::env::var("WATCHDOG_USEC")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|n| *n > 0)
        .map(|n| Duration::from_micros(n / 3));
    let mut heartbeat = watchdog.map(|d| Instant::now() + d);
    let mut fatal = None;
    let mut rescan_at = None;
    let mut reload_at = None;
    loop {
        let setup_deadline = daemon
            .preparing_since
            .values()
            .map(|t| *t + Duration::from_secs(3))
            .min();
        let next = daemon
            .sessions
            .values()
            .filter_map(|s| s.deadline)
            .min()
            .map(|us| epoch + Duration::from_micros(us));
        tokio::select! {
            _=tokio::signal::ctrl_c()=>break,
            _=signal.recv()=>break,
            Some(rpc)=rpcs.recv()=>{
                let message=daemon.rpc(rpc.command).unwrap_or_else(|e|Message::Error {message:format!("{e:#}")});
                let _=rpc.reply.send(message);
            }
            Some(incoming)=inputs.recv()=>match incoming {
                Incoming::Input {id,generation,input} if daemon.sessions.get(&id).is_some_and(|s|s.generation==generation)=>{
                    if let Err(error)=daemon.process(&id,input){daemon.notice(format!("Erro na saída: {error:#}"));daemon.stop(&id);rescan_at=Some(Instant::now()+Duration::from_secs(1));}
                }
                Incoming::Gone {id,generation} if daemon.sessions.get(&id).is_some_and(|s|s.generation==generation)=>{
                    daemon.stop(&id);daemon.publish();rescan_at=Some(Instant::now()+Duration::from_millis(100));
                }
                Incoming::Reload=>reload_at=Some(Instant::now()+Duration::from_millis(150)),
                Incoming::Processes(names)=>{daemon.processes=names;if let Err(error)=daemon.automatic(){daemon.notice(error.to_string());}},
                Incoming::Fatal(error)=>{fatal=Some(anyhow::anyhow!(error));break;},
                Incoming::Discovered(result)=>{
                    discovering=false;
                    match result {Ok(found)=>daemon.reconcile(found,&args.device),Err(error)=>daemon.notice(format!("Hotplug: {error}"))}
                    if scan_again {scan_again=false;rescan_at=Some(Instant::now()+Duration::from_millis(100));}
                }
                Incoming::Prepared {info,token,result}=>{
                    if daemon.preparing.remove(&info.id)!=Some(token){continue;}
                    daemon.preparing_since.remove(&info.id);
                    if daemon.desired.get(&info.id).is_none_or(|d|d.nodes!=info.nodes) {drop(result);rescan_at=Some(Instant::now()+Duration::from_millis(100));continue;}
                    match result {
                        Ok((gamepad,nodes))=>daemon.attach(*info,gamepad,nodes,token),
                        Err(error)=>{
                            daemon.notice(format!("Não foi possível gerenciar {}: {error}",info.name));
                            daemon.errors.insert(info.id.clone(),ControllerStatus {device:*info,profile:daemon.default_profile.clone(),paused:false,error:Some(error),recording:false,automatic:false,layers:Vec::new()});
                        }
                    }
                    daemon.publish();
                }
                _=>{}
            },
            change=changes.recv()=>{if change.is_some(){rescan_at=Some(Instant::now()+Duration::from_millis(100));}else{fatal=Some(anyhow::anyhow!("Monitor udev encerrado"));break;}},
            _=until(setup_deadline)=>{
                daemon.notice("Preparação de dispositivo excedeu 3 s; liberando controles e encerrando para recuperação");
                let ids:Vec<_>=daemon.sessions.keys().cloned().collect();for id in ids {daemon.stop(&id);}
                // A stuck blocking setup task cannot be aborted by Tokio. Exiting
                // closes its descriptors too; systemd restarts the daemon.
                std::process::exit(1);
            },
            _=until(rescan_at)=>{rescan_at=None;if discovering {scan_again=true;}else{discovering=true;scan(daemon.input.clone());}},
            _=until(reload_at)=>{reload_at=None;if let Err(e)=daemon.reload(){daemon.notice(format!("Perfil rejeitado; configuração anterior preservada: {e:#}"));}},
            _=until(next)=>{
                let now=daemon.now();
                let ids:Vec<_>=daemon.sessions.iter().filter(|(_,s)|s.deadline.is_some_and(|d|d<=now)).map(|(id,_)|id.clone()).collect();
                for id in ids {if let Err(e)=daemon.process(&id,Input::Tick){daemon.notice(e.to_string());daemon.stop(&id);}}
            },
            _=until(heartbeat)=>{systemd_notify("WATCHDOG=1");heartbeat=watchdog.map(|d|Instant::now()+d);}
        }
    }
    systemd_notify("STOPPING=1");
    let ids: Vec<_> = daemon.sessions.keys().cloned().collect();
    for id in ids {
        daemon.stop(&id);
    }
    tokio::task::yield_now().await;
    let _ = fs::remove_file(socket);
    if let Some(error) = fatal {
        return Err(error);
    }
    Ok(())
}
