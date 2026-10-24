use anyhow::{Context, Result, bail};
use padsmith_core::Profile;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{path::PathBuf, time::Duration};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

pub const VERSION: u32 = 2;
pub const MAX_MESSAGE: usize = 1024 * 1024;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeInfo {
    pub path: String,
    pub role: String,
    pub name: String,
    pub primary: bool,
    pub keys: Vec<String>,
    pub axes: std::collections::BTreeMap<String, AxisInfo>,
    pub vendor: u16,
    pub product: u16,
    pub serial: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AxisInfo {
    pub minimum: i32,
    pub maximum: i32,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub product: String,
    pub driver: String,
    pub serial: String,
    pub physical_path: String,
    pub connection: String,
    pub nodes: Vec<NodeInfo>,
    pub hidraw: Vec<String>,
    pub battery: Option<u8>,
    pub hidraw_hidden: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ControllerStatus {
    pub device: DeviceInfo,
    pub profile: String,
    pub paused: bool,
    pub error: Option<String>,
    pub recording: bool,
    pub automatic: bool,
    pub layers: Vec<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Status {
    pub controllers: Vec<ControllerStatus>,
    pub profiles: Vec<std::sync::Arc<Profile>>,
    pub version: String,
    pub processes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Status,
    SaveProfile {
        profile: Box<Profile>,
    },
    Apply {
        device: Option<String>,
        profile: String,
    },
    Pause {
        device: Option<String>,
        paused: bool,
    },
    Watch {
        device: Option<String>,
    },
    RecordStart {
        device: String,
    },
    RecordStop {
        device: String,
    },
    RunMacro {
        device: String,
        macro_id: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub version: u32,
    pub request: Command,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Message {
    Recording {
        recording: padsmith_core::Recording,
    },
    Status {
        status: Status,
    },
    Ok,
    Error {
        message: String,
    },
    Raw {
        device: String,
        node: String,
        role: String,
        time_us: u64,
        event_type: u16,
        code: u16,
        value: i32,
        label: String,
    },
    Dropped {
        count: u64,
    },
    Notice {
        message: String,
    },
}

pub fn config_dir() -> Result<PathBuf> {
    Ok(if let Some(path) = std::env::var_os("XDG_CONFIG_HOME") {
        PathBuf::from(path)
    } else {
        PathBuf::from(std::env::var_os("HOME").context("HOME não definido")?).join(".config")
    }
    .join("padsmith"))
}
pub fn socket_path() -> Result<PathBuf> {
    Ok(PathBuf::from(
        std::env::var_os("XDG_RUNTIME_DIR")
            .context("XDG_RUNTIME_DIR não definido; execute na sessão do usuário")?,
    )
    .join("padsmith/control.sock"))
}
pub async fn write_message<T: Serialize>(stream: &mut UnixStream, value: &T) -> Result<()> {
    let mut data = serde_json::to_vec(value)?;
    if data.len() > MAX_MESSAGE {
        bail!("Mensagem IPC excede o limite");
    }
    data.push(b'\n');
    tokio::time::timeout(Duration::from_secs(3), stream.write_all(&data)).await??;
    Ok(())
}
pub async fn read_message<T: DeserializeOwned>(reader: &mut BufReader<UnixStream>) -> Result<T> {
    let mut data = Vec::new();
    let count = reader
        .take((MAX_MESSAGE + 1) as u64)
        .read_until(b'\n', &mut data)
        .await?;
    if count == 0 {
        bail!("Conexão IPC encerrada");
    }
    if count > MAX_MESSAGE || data.last() != Some(&b'\n') {
        bail!("Mensagem IPC inválida ou muito grande");
    }
    Ok(serde_json::from_slice(&data)?)
}
pub async fn connect(command: Command) -> Result<BufReader<UnixStream>> {
    let path = socket_path()?;
    let mut stream = UnixStream::connect(&path)
        .await
        .with_context(|| format!("Daemon indisponível em {}", path.display()))?;
    write_message(
        &mut stream,
        &Request {
            version: VERSION,
            request: command,
        },
    )
    .await?;
    Ok(BufReader::new(stream))
}
pub async fn request(command: Command) -> Result<Message> {
    let mut reader = connect(command).await?;
    let message = tokio::time::timeout(Duration::from_secs(5), read_message(&mut reader)).await??;
    if let Message::Error { message } = &message {
        bail!("{message}");
    }
    Ok(message)
}
