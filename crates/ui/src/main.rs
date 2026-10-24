use padsmith_core::Profile;
use padsmith_protocol::{Command, Message};
use tauri::Emitter;
use tauri_plugin_dialog::DialogExt;
mod appimage;
mod tray;

#[tauri::command]
async fn daemon_request(request: Command) -> Result<Message, String> {
    padsmith_protocol::request(request)
        .await
        .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
async fn doctor() -> Result<Vec<padsmith_linux::doctor::Check>, String> {
    let status = match padsmith_protocol::request(Command::Status).await {
        Ok(Message::Status { status }) => Some(status),
        _ => None,
    };
    tokio::task::spawn_blocking(move || padsmith_linux::doctor::run(status.as_ref(), true))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
async fn import_profile(app: tauri::AppHandle) -> Result<Option<Profile>, String> {
    let (send, receive) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("Perfil Padsmith", &["toml"])
        .pick_file(move |file| {
            let _ = send.send(file);
        });
    let Some(file) = receive.await.map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    let path = file.into_path().map_err(|e| e.to_string())?;
    let meta = tokio::fs::metadata(&path)
        .await
        .map_err(|e| e.to_string())?;
    if meta.len() > padsmith_protocol::MAX_MESSAGE as u64 {
        return Err("Perfil excede 1 MiB".into());
    }
    let text = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| e.to_string())?;
    let profile: Profile = toml::from_str(&text).map_err(|e| e.to_string())?;
    padsmith_linux::validate_profile(&profile).map_err(|e| e.to_string())?;
    padsmith_protocol::request(Command::SaveProfile {
        profile: Box::new(profile.clone()),
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(Some(profile))
}
#[tauri::command]
async fn export_profile(app: tauri::AppHandle, profile: Profile) -> Result<bool, String> {
    padsmith_linux::validate_profile(&profile).map_err(|e| e.to_string())?;
    let (send, receive) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_file_name(format!("{}.toml", profile.id))
        .add_filter("Perfil Padsmith", &["toml"])
        .save_file(move |file| {
            let _ = send.send(file);
        });
    let Some(file) = receive.await.map_err(|e| e.to_string())? else {
        return Ok(false);
    };
    let path = file.into_path().map_err(|e| e.to_string())?;
    let text = toml::to_string_pretty(&profile).map_err(|e| e.to_string())?;
    tokio::task::spawn_blocking(move || {
        padsmith_linux::profiles::atomic_write(&path, text.as_bytes())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    Ok(true)
}
fn main() {
    match appimage::dispatch() {
        Ok(true) => return,
        Ok(false) => {}
        Err(error) => {
            eprintln!("{error:#}");
            std::process::exit(1);
        }
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if !args.iter().any(|a| a == "--tray") {
                tray::show(app);
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            daemon_request,
            doctor,
            import_profile,
            export_profile
        ])
        .setup(|app| {
            let tray = tray::Tray::new(app.handle())?;
            if !std::env::args().any(|a| a == "--tray") {
                tray::show(app.handle());
            }
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                match appimage::start_daemon().await {
                    Ok(Some(mut child)) => {
                        let notices = handle.clone();
                        tauri::async_runtime::spawn(async move {
                            if let Err(error) = child.wait().await.and_then(|status| {
                                if status.success() { Ok(()) } else {
                                    Err(std::io::Error::other(format!("daemon encerrou com {status}")))
                                }
                            }) {
                                let _ = notices.emit("padsmith", Message::Notice {
                                    message: format!("Daemon do AppImage: {error}. Consulte ~/.local/state/padsmith/appimage-daemon.log."),
                                });
                            }
                        });
                    }
                    Ok(None) => {}
                    Err(error) => {
                        eprintln!("Daemon do AppImage: {error:#}");
                        let _ = handle.emit("padsmith", Message::Notice {
                            message: format!("Não foi possível iniciar o daemon do AppImage: {error:#}"),
                        });
                    }
                }
                loop {
                    if let Ok(mut reader) =
                        padsmith_protocol::connect(Command::Watch { device: None }).await
                    {
                        while let Ok(message) =
                            padsmith_protocol::read_message::<Message>(&mut reader).await
                        {
                            if let Message::Status { status } = &message {
                                let _ = tray.update(status);
                            }
                            let _ = handle.emit("padsmith", message);
                        }
                    }
                    let _ = handle.emit("padsmith-offline", ());
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("Não foi possível iniciar a interface Padsmith");
}
