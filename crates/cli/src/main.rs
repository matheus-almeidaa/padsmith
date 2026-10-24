use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use padsmith_protocol::{Command, Message};
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about = "Padsmith — controles do seu jeito")]
struct Args {
    #[command(subcommand)]
    command: Action,
}
#[derive(Subcommand)]
enum Action {
    #[command(subcommand)]
    Record(RecordAction),
    /// Diagnostica permissões, KWin, regras udev e captura exclusiva.
    Doctor {
        #[arg(long)]
        json: bool,
        #[arg(long)]
        no_probe: bool,
    },
    /// Lista dispositivos físicos, inclusive sem o daemon.
    Devices {
        #[arg(long)]
        json: bool,
    },
    Status,
    Pause {
        #[arg(long)]
        device: Option<String>,
    },
    Resume {
        #[arg(long)]
        device: Option<String>,
    },
    /// Exibe eventos crus de todos os nós do controle, através do daemon.
    Monitor {
        #[arg(long)]
        device: Option<String>,
        #[arg(long)]
        json: bool,
    },
    #[command(subcommand)]
    Profile(ProfileAction),
}
#[derive(Subcommand)]
enum RecordAction {
    Start { device: String },
    Stop { device: String },
}
#[derive(Subcommand)]
enum ProfileAction {
    List,
    Apply {
        id: String,
        #[arg(long)]
        device: Option<String>,
    },
    Import {
        path: PathBuf,
    },
    Export {
        id: String,
        path: PathBuf,
    },
    /// Grava um perfil inicial editável. --swap-xy inclui X ↔ Y.
    Create {
        id: String,
        #[arg(long)]
        swap_xy: bool,
    },
}
fn print_message(message: &Message) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(message)?);
    Ok(())
}
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args = Args::parse();
    match args.command {
        Action::Record(RecordAction::Start { device }) => {
            padsmith_protocol::request(Command::RecordStart { device }).await?;
            println!(
                "Gravação iniciada; eventos do controle não são enviados aos dispositivos virtuais."
            );
        }
        Action::Record(RecordAction::Stop { device }) => {
            print_message(&padsmith_protocol::request(Command::RecordStop { device }).await?)?
        }
        Action::Doctor { json, no_probe } => {
            let status = match padsmith_protocol::request(Command::Status).await {
                Ok(Message::Status { status }) => Some(status),
                _ => None,
            };
            let checks = padsmith_linux::doctor::run(status.as_ref(), !no_probe)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&checks)?);
            } else {
                for check in &checks {
                    println!(
                        "[{:7}] {}\n          {}",
                        check.level, check.subject, check.detail
                    );
                }
            }
            if checks.iter().any(|c| c.level == "error") {
                bail!("Diagnóstico encontrou erros");
            }
        }
        Action::Devices { json } => {
            let devices = padsmith_linux::devices::discover()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&devices)?);
            } else {
                for d in devices {
                    println!(
                        "{} — {}:{} [{}]\n  ID: {}",
                        d.name, d.vendor, d.product, d.driver, d.id
                    );
                    for n in d.nodes {
                        println!("  {} {:10} {}", n.path, n.role, n.name);
                    }
                    for n in d.hidraw {
                        println!("  {n} hidraw");
                    }
                }
            }
        }
        Action::Status => print_message(&padsmith_protocol::request(Command::Status).await?)?,
        Action::Pause { device } => {
            padsmith_protocol::request(Command::Pause {
                device,
                paused: true,
            })
            .await?;
            println!("Remapeamento pausado; passagem direta ativa.");
        }
        Action::Resume { device } => {
            padsmith_protocol::request(Command::Pause {
                device,
                paused: false,
            })
            .await?;
            println!("Remapeamento retomado.");
        }
        Action::Monitor { device, json } => {
            let mut connection = padsmith_protocol::connect(Command::Watch { device }).await?;
            loop {
                let message: Message = padsmith_protocol::read_message(&mut connection).await?;
                if json {
                    println!("{}", serde_json::to_string(&message)?);
                } else {
                    match message {
                        Message::Raw {
                            time_us,
                            node,
                            role,
                            event_type,
                            code,
                            value,
                            label,
                            ..
                        } => println!(
                            "{time_us:12} {node:20} {role:10} tipo={event_type:2} código={code:4} valor={value:8} {label}"
                        ),
                        Message::Dropped { count } => eprintln!(
                            "Monitor perdeu {count} eventos; saída de controle continua independente"
                        ),
                        Message::Notice { message } | Message::Error { message } => {
                            eprintln!("{message}")
                        }
                        _ => {}
                    }
                }
            }
        }
        Action::Profile(action) => match action {
            ProfileAction::Apply { id, device } => {
                padsmith_protocol::request(Command::Apply {
                    device,
                    profile: id,
                })
                .await?;
            }
            ProfileAction::Import { path } => {
                let profile: padsmith_core::Profile =
                    toml::from_str(&std::fs::read_to_string(path)?)?;
                padsmith_protocol::request(Command::SaveProfile {
                    profile: Box::new(profile),
                })
                .await?;
            }
            ProfileAction::List => {
                if let Message::Status { status } =
                    padsmith_protocol::request(Command::Status).await?
                {
                    for p in status.profiles {
                        println!("{}\t{}", p.id, p.name);
                    }
                }
            }
            ProfileAction::Export { id, path } => {
                if let Message::Status { status } =
                    padsmith_protocol::request(Command::Status).await?
                {
                    let profile = status
                        .profiles
                        .iter()
                        .find(|p| p.id == id)
                        .ok_or_else(|| anyhow::anyhow!("Perfil não encontrado"))?;
                    anyhow::ensure!(!path.exists(), "Arquivo de destino já existe");
                    padsmith_linux::profiles::atomic_write(
                        &path,
                        toml::to_string_pretty(profile)?.as_bytes(),
                    )?;
                }
            }
            ProfileAction::Create { id, swap_xy } => {
                let mut profile = padsmith_core::Profile {
                    name: id.clone(),
                    id,
                    ..Default::default()
                };
                if swap_xy {
                    for (a, b) in [("BTN_WEST", "BTN_NORTH"), ("BTN_NORTH", "BTN_WEST")] {
                        profile.buttons.insert(
                            a.into(),
                            padsmith_core::Binding {
                                output: Some(padsmith_core::Target::Gamepad { code: b.into() }),
                                ..Default::default()
                            },
                        );
                    }
                }
                let directory = padsmith_protocol::config_dir()?.join("profiles");
                anyhow::ensure!(
                    !directory.join(format!("{}.toml", profile.id)).exists(),
                    "Perfil já existe"
                );
                padsmith_linux::profiles::save(&directory, &profile)?;
                println!("Perfil criado em {}", directory.display());
            }
        },
    }
    Ok(())
}
