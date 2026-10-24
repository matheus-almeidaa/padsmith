//! AppImage entry points and an independently mounted daemon process.
use anyhow::{Context, Result};
use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    os::unix::{fs::OpenOptionsExt, process::CommandExt},
    path::{Path, PathBuf},
    process::Stdio,
};
use tokio::{net::UnixStream, process::Child};

fn packaged_image() -> Option<OsString> {
    std::env::var_os("APPDIR")?;
    std::env::var_os("APPIMAGE")
}

/// Dispatch before the single-instance plugin: CLI/daemon calls must not open a window.
pub fn dispatch() -> Result<bool> {
    if packaged_image().is_none() {
        return Ok(false);
    }
    let mut args = std::env::args_os().skip(1);
    let program = match args.next().as_deref().and_then(|arg| arg.to_str()) {
        Some("--daemon") => "padsmithd",
        Some("--cli") => "padsmith",
        Some("--help" | "-h") => {
            println!(
                "Padsmith AppImage\n\nSem argumentos: abre a interface e inicia o daemon se necessário.\n\
                 --cli <comando>  Executa a CLI (ex.: --cli status).\n\
                 --daemon         Executa somente o daemon.\n\
                 --tray           Abre somente a bandeja."
            );
            return Ok(true);
        }
        _ => return Ok(false),
    };
    let executable = std::env::current_exe()?;
    let binary = executable
        .parent()
        .context("Diretório do executável não encontrado")?
        .join(program);
    let error = std::process::Command::new(&binary).args(args).exec();
    Err(error).with_context(|| format!("Não foi possível executar {}", binary.display()))
}

pub async fn start_daemon() -> Result<Option<Child>> {
    let Some(image) = packaged_image() else {
        return Ok(None);
    };
    let state = if let Some(path) = std::env::var_os("XDG_STATE_HOME") {
        PathBuf::from(path)
    } else {
        PathBuf::from(std::env::var_os("HOME").context("HOME não definido")?).join(".local/state")
    };
    start_at(
        &padsmith_protocol::socket_path()?,
        Path::new(&image),
        &state.join("padsmith/appimage-daemon.log"),
    )
    .await
}

async fn start_at(socket: &Path, image: &Path, log_path: &Path) -> Result<Option<Child>> {
    // A live daemon, including one managed by systemd, retains ownership of the device.
    if UnixStream::connect(socket).await.is_ok() {
        return Ok(None);
    }
    fs::create_dir_all(log_path.parent().context("Diretório de log inválido")?)?;
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(log_path)?;
    let mut command = tokio::process::Command::new(image);
    command
        .arg("--daemon")
        .env_remove("APPDIR")
        .env_remove("APPIMAGE")
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .kill_on_drop(false);
    command.as_std_mut().process_group(0);
    // Re-enter the original AppImage, giving the daemon its own mount and lifetime.
    // Executing usr/bin/padsmithd from the UI's mount would break after the UI exits.
    command
        .spawn()
        .with_context(|| format!("Falha ao iniciar o daemon; consulte {}", log_path.display()))
        .map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        os::unix::fs::PermissionsExt,
        sync::atomic::{AtomicUsize, Ordering},
    };

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "padsmith-appimage-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn script(&self, text: &str) -> PathBuf {
            let path = self.0.join("fake.AppImage");
            fs::write(&path, text).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
            path
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[tokio::test]
    async fn reuses_a_live_daemon_without_launching_or_creating_logs() -> Result<()> {
        let fixture = Fixture::new();
        let socket = fixture.0.join("control.sock");
        let _listener = tokio::net::UnixListener::bind(&socket)?;
        let log = fixture.0.join("unused/log");
        assert!(
            start_at(&socket, Path::new("/nonexistent.AppImage"), &log)
                .await?
                .is_none()
        );
        assert!(!log.exists());
        Ok(())
    }

    #[tokio::test]
    async fn launches_the_original_image_in_daemon_mode_and_records_output() -> Result<()> {
        let fixture = Fixture::new();
        let image = fixture.script("#!/bin/sh\nprintf '%s' \"$1\"\n");
        let log = fixture.0.join("logs/daemon.log");
        let mut child = start_at(&fixture.0.join("missing.sock"), &image, &log)
            .await?
            .unwrap();
        assert!(child.wait().await?.success());
        assert_eq!(fs::read_to_string(log)?, "--daemon");
        Ok(())
    }

    #[tokio::test]
    async fn dropping_the_ui_child_handle_does_not_kill_the_daemon() -> Result<()> {
        let fixture = Fixture::new();
        let image = fixture.script("#!/bin/sh\nsleep 0.1\nprintf 'still alive'\n");
        let log = fixture.0.join("daemon.log");
        drop(start_at(&fixture.0.join("missing.sock"), &image, &log).await?);
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            loop {
                if fs::read_to_string(&log).unwrap_or_default() == "still alive" {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        })
        .await?;
        Ok(())
    }
}
