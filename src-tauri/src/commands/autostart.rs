//! Launch-at-startup (OS autostart) commands.
//!
//! Thin wrappers over `tauri-plugin-autostart` so the General settings toggle
//! can read and change the OS-level "start on login" registration (Windows
//! registry Run key, macOS LaunchAgent, Linux autostart `.desktop`). The plugin
//! is desktop-only; on other targets these commands are inert no-ops.

/// Whether Pebble is currently registered to launch when the user logs in.
#[tauri::command]
pub fn get_autostart_enabled(app: tauri::AppHandle) -> Result<bool, String> {
    #[cfg(desktop)]
    {
        use tauri_plugin_autostart::ManagerExt;
        app.autolaunch().is_enabled().map_err(|e| e.to_string())
    }
    #[cfg(not(desktop))]
    {
        let _ = app;
        Ok(false)
    }
}

/// Enable or disable launching Pebble when the user logs in.
#[tauri::command]
pub fn set_autostart_enabled(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    #[cfg(desktop)]
    {
        use tauri_plugin_autostart::ManagerExt;
        let manager = app.autolaunch();
        if enabled {
            manager.enable().map_err(|e| e.to_string())
        } else {
            manager.disable().map_err(|e| e.to_string())
        }
    }
    #[cfg(not(desktop))]
    {
        let _ = (app, enabled);
        Ok(())
    }
}
