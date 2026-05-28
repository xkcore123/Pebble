#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    configure_linux_appimage_webkit_runtime();
    pebble_lib::run();
}

fn configure_linux_appimage_webkit_runtime() {
    #[cfg(target_os = "linux")]
    {
        // Some WebKitGTK/AppImage/Wayland combinations miss repaint updates unless accelerated
        // compositing is disabled before WebKit initializes.
        if std::env::var_os("APPIMAGE").is_some()
            && std::env::var_os("WEBKIT_DISABLE_COMPOSITING_MODE").is_none()
        {
            std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        }
    }
}
