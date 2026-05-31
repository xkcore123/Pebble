use crate::commands::kanban::{
    encrypt_kanban_context_notes_for_state, load_kanban_context_notes_for_state,
    KANBAN_CONTEXT_NOTES_KEY,
};
use crate::commands::translate::{decrypt_config as decrypt_translate_config, encrypt_config};
use crate::state::AppState;
use pebble_core::PebbleError;
use pebble_crypto::passphrase::{
    decrypt_with_passphrase, encrypt_with_passphrase, PassphraseEncryptedBlob,
};
use pebble_store::cloud_sync::{
    preview_backup, BackupPreview, BackupSecretSummary, RestoredAuthData, RestoredPrivateData,
    RestoredSecureUserData, SettingsBackup, WebDavClient, SETTINGS_BACKUP_FILENAME,
};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AccountAuthBackup {
    account_id: String,
    provider: String,
    auth_data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct BackupSecrets {
    #[serde(default)]
    account_auth: Vec<AccountAuthBackup>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    translate_config: Option<String>,
}

impl BackupSecrets {
    fn is_empty(&self) -> bool {
        self.account_auth.is_empty() && self.translate_config.is_none()
    }
}

fn secret_summary(secrets: &BackupSecrets) -> BackupSecretSummary {
    BackupSecretSummary {
        account_auth_count: secrets.account_auth.len(),
        has_translate_config: secrets.translate_config.is_some(),
    }
}

fn encrypt_backup_secrets(
    secrets: &BackupSecrets,
    passphrase: &str,
) -> std::result::Result<PassphraseEncryptedBlob, PebbleError> {
    let plaintext = serde_json::to_vec(secrets)
        .map_err(|e| PebbleError::Internal(format!("Failed to serialize backup secrets: {e}")))?;
    encrypt_with_passphrase(&plaintext, passphrase)
}

fn decrypt_backup_secrets(
    encrypted: &PassphraseEncryptedBlob,
    passphrase: &str,
) -> std::result::Result<BackupSecrets, PebbleError> {
    let plaintext = decrypt_with_passphrase(encrypted, passphrase)?;
    serde_json::from_slice(&plaintext)
        .map_err(|e| PebbleError::Validation(format!("Failed to parse backup secrets: {e}")))
}

fn collect_backup_secrets(state: &AppState) -> std::result::Result<BackupSecrets, PebbleError> {
    let mut account_auth = Vec::new();
    for account in state.store.list_accounts()? {
        let Some(encrypted) = state.store.get_auth_data(&account.id)? else {
            continue;
        };
        let decrypted = state.crypto.decrypt(&encrypted)?;
        let auth_data = serde_json::from_slice(&decrypted).map_err(|e| {
            PebbleError::Internal(format!(
                "Failed to parse decrypted auth data for {}: {e}",
                account.email
            ))
        })?;
        account_auth.push(AccountAuthBackup {
            account_id: account.id,
            provider: provider_slug(&account.provider).to_string(),
            auth_data,
        });
    }

    let translate_config = state
        .store
        .get_translate_config()?
        .map(|tc| decrypt_translate_config(state, &tc.config))
        .transpose()?;

    Ok(BackupSecrets {
        account_auth,
        translate_config,
    })
}

fn attach_encrypted_secrets(
    state: &AppState,
    backup: &mut SettingsBackup,
    secret_passphrase: Option<String>,
) -> std::result::Result<(), PebbleError> {
    let Some(passphrase) = secret_passphrase else {
        return Ok(());
    };
    let secrets = collect_backup_secrets(state)?;
    if secrets.is_empty() {
        return Ok(());
    }
    let encrypted = encrypt_backup_secrets(&secrets, &passphrase)?;
    backup.secret_summary = Some(secret_summary(&secrets));
    backup.encrypted_secrets = Some(serde_json::to_value(encrypted).map_err(|e| {
        PebbleError::Internal(format!("Failed to serialize encrypted backup secrets: {e}"))
    })?);
    Ok(())
}

fn decrypt_secrets_from_backup(
    backup: &SettingsBackup,
    secret_passphrase: Option<String>,
) -> std::result::Result<Option<BackupSecrets>, PebbleError> {
    let Some(value) = &backup.encrypted_secrets else {
        return Ok(None);
    };
    let Some(passphrase) = secret_passphrase else {
        return Err(PebbleError::Validation(
            "This backup contains encrypted account passwords, OAuth tokens, or API keys. Enter the backup encryption password to restore them.".to_string(),
        ));
    };
    let encrypted: PassphraseEncryptedBlob =
        serde_json::from_value(value.clone()).map_err(|e| {
            PebbleError::Validation(format!("Invalid encrypted backup secret payload: {e}"))
        })?;
    decrypt_backup_secrets(&encrypted, &passphrase).map(Some)
}

fn prepare_restored_private_data(
    state: &AppState,
    backup: &SettingsBackup,
    has_kanban_context_notes: bool,
    secrets: Option<BackupSecrets>,
) -> std::result::Result<RestoredPrivateData, PebbleError> {
    let mut private_data = RestoredPrivateData::default();

    if has_kanban_context_notes {
        private_data.secure_user_data.push(RestoredSecureUserData {
            key: KANBAN_CONTEXT_NOTES_KEY.to_string(),
            encrypted: encrypt_kanban_context_notes_for_state(
                state,
                backup.kanban_context_notes.clone(),
            )?,
        });
    }

    let Some(secrets) = secrets else {
        return Ok(private_data);
    };

    for account in secrets.account_auth {
        let auth_bytes = serde_json::to_vec(&account.auth_data).map_err(|e| {
            PebbleError::Internal(format!(
                "Failed to serialize restored auth data for account {}: {e}",
                account.account_id
            ))
        })?;
        let encrypted = state.crypto.encrypt(&auth_bytes)?;
        private_data.auth_data.push(RestoredAuthData {
            account_id: account.account_id,
            provider: account.provider,
            encrypted,
        });
    }

    if let (Some(secret_config), Some(mut translate_config)) =
        (secrets.translate_config, backup.translate_config.clone())
    {
        translate_config.config = encrypt_config(state, &secret_config)?;
        private_data.translate_config = Some(translate_config);
    }

    Ok(private_data)
}

fn provider_slug(provider: &pebble_core::ProviderType) -> &'static str {
    match provider {
        pebble_core::ProviderType::Imap => "imap",
        pebble_core::ProviderType::Pop3 => "pop3",
        pebble_core::ProviderType::Gmail => "gmail",
        pebble_core::ProviderType::Outlook => "outlook",
    }
}

fn build_backup_data(
    state: &AppState,
    secret_passphrase: Option<String>,
) -> std::result::Result<Vec<u8>, PebbleError> {
    let exported = state.store.export_settings()?;
    let mut backup: SettingsBackup = serde_json::from_slice(&exported)
        .map_err(|e| PebbleError::Internal(format!("Failed to build backup payload: {e}")))?;
    backup.kanban_context_notes = load_kanban_context_notes_for_state(state)?;
    attach_encrypted_secrets(state, &mut backup, secret_passphrase)?;
    serde_json::to_vec_pretty(&backup)
        .map_err(|e| PebbleError::Internal(format!("Failed to serialize backup payload: {e}")))
}

fn restore_backup_data(
    state: &AppState,
    data: &[u8],
    secret_passphrase: Option<String>,
) -> std::result::Result<String, PebbleError> {
    // Re-validate before import; `import_settings` enforces size + version too.
    let _ = preview_backup(data)?;
    let backup_value: serde_json::Value = serde_json::from_slice(data)
        .map_err(|e| PebbleError::Validation(format!("Failed to parse backup: {e}")))?;
    let has_kanban_context_notes = backup_value.get("kanban_context_notes").is_some();
    let backup: SettingsBackup = serde_json::from_value(backup_value)
        .map_err(|e| PebbleError::Validation(format!("Failed to parse backup: {e}")))?;
    let backup_secrets = decrypt_secrets_from_backup(&backup, secret_passphrase)?;
    let restored_secrets = backup_secrets.is_some();
    let private_data =
        prepare_restored_private_data(state, &backup, has_kanban_context_notes, backup_secrets)?;
    state
        .store
        .import_settings_with_private_data(data, private_data)?;
    if restored_secrets {
        return Ok(
            "Settings backup restored with account passwords, OAuth tokens, and API keys."
                .to_string(),
        );
    }
    Ok("Settings backup restored. Reconnect accounts to continue syncing.".to_string())
}

#[tauri::command]
pub async fn test_webdav_connection(
    url: String,
    username: String,
    password: String,
) -> std::result::Result<String, PebbleError> {
    let client = WebDavClient::new(url, username, password)?;
    client.test_connection().await?;
    Ok("Connection successful".to_string())
}

#[tauri::command]
pub async fn backup_to_webdav(
    state: State<'_, AppState>,
    url: String,
    username: String,
    password: String,
    secret_passphrase: Option<String>,
) -> std::result::Result<String, PebbleError> {
    let data = build_backup_data(&state, secret_passphrase)?;
    let client = WebDavClient::new(url, username, password)?;
    client.upload(SETTINGS_BACKUP_FILENAME, &data).await?;
    Ok("Settings backup completed successfully".to_string())
}

#[tauri::command]
pub fn export_backup_file(
    state: State<'_, AppState>,
    secret_passphrase: Option<String>,
) -> std::result::Result<String, PebbleError> {
    let data = build_backup_data(&state, secret_passphrase)?;
    String::from_utf8(data)
        .map_err(|e| PebbleError::Internal(format!("Backup JSON was not valid UTF-8: {e}")))
}

#[tauri::command]
pub fn preview_backup_file(data: String) -> std::result::Result<BackupPreview, PebbleError> {
    preview_backup(data.as_bytes())
}

#[tauri::command]
pub fn import_backup_file(
    state: State<'_, AppState>,
    data: String,
    secret_passphrase: Option<String>,
) -> std::result::Result<String, PebbleError> {
    restore_backup_data(&state, data.as_bytes(), secret_passphrase)
}

/// Download the backup and return a summary so the user can review the
/// contents before committing to a restore. Enforces size limits and schema
/// version validation in `download` and `preview_backup`.
#[tauri::command]
pub async fn preview_webdav_backup(
    url: String,
    username: String,
    password: String,
) -> std::result::Result<BackupPreview, PebbleError> {
    let client = WebDavClient::new(url, username, password)?;
    let data = client.download(SETTINGS_BACKUP_FILENAME).await?;
    preview_backup(&data)
}

#[tauri::command]
pub async fn restore_from_webdav(
    state: State<'_, AppState>,
    url: String,
    username: String,
    password: String,
    secret_passphrase: Option<String>,
) -> std::result::Result<String, PebbleError> {
    let client = WebDavClient::new(url, username, password)?;
    let data = client.download(SETTINGS_BACKUP_FILENAME).await?;
    restore_backup_data(&state, &data, secret_passphrase)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypted_backup_secrets_round_trip_without_plaintext_leaks() {
        let secrets = BackupSecrets {
            account_auth: vec![AccountAuthBackup {
                account_id: "imap-account".to_string(),
                provider: "imap".to_string(),
                auth_data: serde_json::json!({
                    "imap": { "password": "imap-secret" },
                    "smtp": { "password": "smtp-secret" }
                }),
            }],
            translate_config: Some(r#"{"type":"deepl","api_key":"deepl-secret"}"#.to_string()),
        };

        let encrypted = encrypt_backup_secrets(&secrets, "backup passphrase").unwrap();
        let serialized = serde_json::to_string(&encrypted).unwrap();

        assert!(!serialized.contains("imap-secret"));
        assert!(!serialized.contains("smtp-secret"));
        assert!(!serialized.contains("deepl-secret"));

        let decrypted = decrypt_backup_secrets(&encrypted, "backup passphrase").unwrap();
        assert_eq!(decrypted, secrets);
    }

    #[test]
    fn backup_secret_summary_counts_accounts_and_translate_config() {
        let secrets = BackupSecrets {
            account_auth: vec![
                AccountAuthBackup {
                    account_id: "a1".to_string(),
                    provider: "imap".to_string(),
                    auth_data: serde_json::json!({"password":"one"}),
                },
                AccountAuthBackup {
                    account_id: "a2".to_string(),
                    provider: "gmail".to_string(),
                    auth_data: serde_json::json!({"access_token":"token"}),
                },
            ],
            translate_config: Some(r#"{"api_key":"key"}"#.to_string()),
        };

        let summary = secret_summary(&secrets);

        assert_eq!(summary.account_auth_count, 2);
        assert!(summary.has_translate_config);
    }

    #[test]
    fn preview_backup_file_accepts_backup_json_text() {
        let backup = serde_json::json!({
            "version": 1,
            "exported_at": 1,
            "accounts": [],
            "rules": [],
            "kanban_cards": [],
            "kanban_context_notes": {},
            "translate_config": null
        });

        let preview = preview_backup_file(backup.to_string()).unwrap();

        assert_eq!(preview.version, 1);
        assert_eq!(preview.account_count, 0);
    }
}
