use padsmith_protocol::{Command, Status};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use tauri::{
    AppHandle, Manager,
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
};

type Actions = Arc<Mutex<BTreeMap<String, Command>>>;
pub fn show(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}
pub struct Tray {
    app: AppHandle,
    actions: Actions,
}
impl Tray {
    pub fn new(app: &AppHandle) -> tauri::Result<Self> {
        let actions = Actions::default();
        let callback_actions = actions.clone();
        let icon = app.default_window_icon().cloned().unwrap_or_else(|| {
            tauri::image::Image::new_owned([186, 242, 102, 255].repeat(32 * 32), 32, 32)
        });
        TrayIconBuilder::with_id("padsmith")
            .icon(icon)
            .tooltip("Padsmith — controles do seu jeito")
            .show_menu_on_left_click(true)
            .on_menu_event(move |app, event| {
                let id = event.id().as_ref();
                if id == "open" {
                    show(app);
                    return;
                }
                if id == "quit" {
                    app.exit(0);
                    return;
                }
                let command = callback_actions
                    .lock()
                    .ok()
                    .and_then(|map| map.get(id).cloned());
                if let Some(command) = command {
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(error) = padsmith_protocol::request(command).await {
                            use tauri::Emitter;
                            let _ = app.emit(
                                "padsmith",
                                padsmith_protocol::Message::Notice {
                                    message: format!("Ação da bandeja não aplicada: {error:#}"),
                                },
                            );
                        }
                    });
                }
            })
            .build(app)?;
        let tray = Self {
            app: app.clone(),
            actions,
        };
        tray.update(&Status::default())?;
        Ok(tray)
    }
    pub fn update(&self, status: &Status) -> tauri::Result<()> {
        let app = &self.app;
        let menu = Menu::new(app)?;
        let mut actions = BTreeMap::new();
        let open = MenuItem::with_id(app, "open", "Abrir Padsmith", true, None::<&str>)?;
        menu.append(&open)?;
        let active: Vec<_> = status
            .controllers
            .iter()
            .filter(|c| c.error.is_none())
            .collect();
        let title = if active.is_empty() {
            "Nenhum controle capturado".into()
        } else {
            active
                .iter()
                .map(|c| {
                    let name = status
                        .profiles
                        .iter()
                        .find(|p| p.id == c.profile)
                        .map(|p| p.name.as_str())
                        .unwrap_or(&c.profile);
                    format!("{}{}", name, if c.paused { " (pausado)" } else { "" })
                })
                .collect::<Vec<_>>()
                .join(" · ")
        };
        menu.append(&MenuItem::with_id(
            app,
            "active",
            &title,
            false,
            None::<&str>,
        )?)?;
        menu.append(&PredefinedMenuItem::separator(app)?)?;
        let all_paused = !active.is_empty() && active.iter().all(|c| c.paused);
        menu.append(&MenuItem::with_id(
            app,
            "pause",
            if all_paused {
                "Retomar remapeamento"
            } else {
                "Pausar remapeamento"
            },
            !active.is_empty(),
            None::<&str>,
        )?)?;
        actions.insert(
            "pause".into(),
            Command::Pause {
                device: None,
                paused: !all_paused,
            },
        );
        for (controller_index, c) in active.iter().enumerate() {
            let controller = Submenu::new(
                app,
                format!(
                    "Controle {} · {}:{}",
                    controller_index + 1,
                    c.device.vendor,
                    c.device.product
                ),
                true,
            )?;
            for (profile_index, p) in status.profiles.iter().enumerate() {
                let id = format!("profile-{controller_index}-{profile_index}");
                controller.append(&MenuItem::with_id(
                    app,
                    &id,
                    format!("{}{}", if c.profile == p.id { "✓ " } else { "" }, p.name),
                    true,
                    None::<&str>,
                )?)?;
                actions.insert(
                    id,
                    Command::Apply {
                        device: Some(c.device.id.clone()),
                        profile: p.id.clone(),
                    },
                );
            }
            menu.append(&controller)?;
        }
        menu.append(&PredefinedMenuItem::separator(app)?)?;
        menu.append(&MenuItem::with_id(
            app,
            "quit",
            "Sair da interface (daemon continua)",
            true,
            None::<&str>,
        )?)?;
        if let Some(tray) = app.tray_by_id("padsmith") {
            tray.set_menu(Some(menu))?;
            tray.set_tooltip(Some(format!("Padsmith — {title}")))?;
        }
        if let Ok(mut map) = self.actions.lock() {
            *map = actions;
        }
        Ok(())
    }
}
