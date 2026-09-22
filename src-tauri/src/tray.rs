use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let title_i = MenuItem::with_id(app, "title", "Kindle Glean", false, None::<&str>)?;
    let sync_i = MenuItem::with_id(app, "sync_now", "今すぐ同期 (Sync Now)", true, None::<&str>)?;
    let show_i = MenuItem::with_id(app, "show", "メイン画面を表示 (Show)", true, None::<&str>)?;
    let sep = MenuItem::with_id(app, "sep", "---", false, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "終了 (Quit)", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&title_i, &sync_i, &show_i, &sep, &quit_i])?;

    let tray = TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "sync_now" => {
                let _ = app.emit("trigger-sync", ());
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        });

    // Use default window icon for tray
    if let Some(icon) = app.default_window_icon() {
        let _ = tray.icon(icon.clone()).build(app);
    }

    Ok(())
}
