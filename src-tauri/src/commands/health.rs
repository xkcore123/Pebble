use crate::state::AppState;
use semver::Version;
use serde::Serialize;
use tauri::State;

#[derive(Serialize)]
pub struct UpdateInfo {
    pub latest_version: String,
    pub release_url: String,
    pub is_newer: bool,
}

#[tauri::command]
pub async fn check_for_update(current_version: String) -> Result<UpdateInfo, String> {
    let client = reqwest::Client::builder()
        .user_agent("Pebble-Email-Client")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let resp = client
        .get("https://api.github.com/repos/QingJ01/Pebble/releases/latest")
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await
        .map_err(|e| format!("Failed to check for updates: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("GitHub API returned status {}", resp.status()));
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {e}"))?;

    let tag = data["tag_name"]
        .as_str()
        .ok_or("Missing tag_name in response")?;
    let latest = tag.trim_start_matches('v').to_string();
    let release_url = data["html_url"]
        .as_str()
        .unwrap_or("https://github.com/QingJ01/Pebble/releases")
        .to_string();

    let has_update = match (Version::parse(&latest), Version::parse(&current_version)) {
        (Ok(latest_ver), Ok(current_ver)) => latest_ver > current_ver,
        _ => latest != current_version,
    };

    Ok(UpdateInfo {
        is_newer: has_update,
        latest_version: latest,
        release_url,
    })
}

#[tauri::command]
pub fn open_default_mail_settings() -> Result<(), String> {
    #[cfg(windows)]
    {
        register_as_mail_client()?;
        opener::open("ms-settings:defaultapps").map_err(|e| format!("Failed to open settings: {e}"))
    }
    #[cfg(not(windows))]
    {
        Err("This feature is only available on Windows".to_string())
    }
}

#[cfg(windows)]
fn register_as_mail_client() -> Result<(), String> {
    use windows_registry::CURRENT_USER;

    let exe_path = tauri::utils::platform::current_exe()
        .map_err(|e| format!("Failed to get exe path: {e}"))?;
    let exe = exe_path.to_string_lossy();
    let open_cmd = format!("\"{exe}\" \"%1\"");

    // Register as a mail client
    let client_key = CURRENT_USER
        .create(r"SOFTWARE\Clients\Mail\Pebble")
        .map_err(|e| e.to_string())?;
    client_key
        .set_string("", "Pebble")
        .map_err(|e| e.to_string())?;

    let shell_key = CURRENT_USER
        .create(r"SOFTWARE\Clients\Mail\Pebble\shell\open\command")
        .map_err(|e| e.to_string())?;
    shell_key
        .set_string("", format!("\"{exe}\""))
        .map_err(|e| e.to_string())?;

    // Register capabilities
    let caps_key = CURRENT_USER
        .create(r"SOFTWARE\Clients\Mail\Pebble\Capabilities")
        .map_err(|e| e.to_string())?;
    caps_key
        .set_string("ApplicationName", "Pebble")
        .map_err(|e| e.to_string())?;
    caps_key
        .set_string("ApplicationDescription", "Pebble Email Client")
        .map_err(|e| e.to_string())?;

    let url_key = CURRENT_USER
        .create(r"SOFTWARE\Clients\Mail\Pebble\Capabilities\UrlAssociations")
        .map_err(|e| e.to_string())?;
    url_key
        .set_string("mailto", "Pebble.Url.mailto")
        .map_err(|e| e.to_string())?;

    // Register mailto protocol handler class
    let class_key = CURRENT_USER
        .create(r"SOFTWARE\Classes\Pebble.Url.mailto")
        .map_err(|e| e.to_string())?;
    class_key
        .set_string("", "Pebble Email URL")
        .map_err(|e| e.to_string())?;
    class_key
        .set_string("URL Protocol", "")
        .map_err(|e| e.to_string())?;

    let class_cmd_key = CURRENT_USER
        .create(r"SOFTWARE\Classes\Pebble.Url.mailto\shell\open\command")
        .map_err(|e| e.to_string())?;
    class_cmd_key
        .set_string("", &open_cmd)
        .map_err(|e| e.to_string())?;

    // Register in RegisteredApplications
    let reg_apps_key = CURRENT_USER
        .create(r"SOFTWARE\RegisteredApplications")
        .map_err(|e| e.to_string())?;
    reg_apps_key
        .set_string("Pebble", r"SOFTWARE\Clients\Mail\Pebble\Capabilities")
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn open_external_url(url: String) -> Result<(), String> {
    // Only allow safe URL schemes to prevent command injection via opener::open / ShellExecuteW
    if !url.starts_with("https://") && !url.starts_with("http://") && !url.starts_with("mailto:") {
        return Err("Only https://, http://, and mailto: URLs are permitted".to_string());
    }
    opener::open(&url).map_err(|e| format!("Failed to open URL: {e}"))
}

#[tauri::command]
pub fn health_check(state: State<'_, AppState>) -> Result<String, String> {
    match state.store.list_accounts() {
        Ok(accounts) => Ok(format!(
            "Pebble is healthy. {} account(s) configured.",
            accounts.len()
        )),
        Err(e) => Err(format!("Health check failed: {}", e)),
    }
}
