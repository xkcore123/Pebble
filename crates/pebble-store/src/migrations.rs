use pebble_core::{build_snippet, PebbleError, Result};
use rusqlite::{Connection, OptionalExtension};
use std::collections::HashSet;

const CURRENT_VERSION: u32 = 13;
const ACCOUNT_COLOR_PRESETS: [&str; 12] = [
    "#0ea5e9", "#22c55e", "#f59e0b", "#8b5cf6", "#f43f5e", "#14b8a6", "#6366f1", "#f97316",
    "#06b6d4", "#ec4899", "#84cc16", "#3b82f6",
];

fn get_schema_version(conn: &Connection) -> u32 {
    conn.pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap_or(0)
}

fn set_schema_version(conn: &Connection, version: u32) -> Result<()> {
    conn.pragma_update(None, "user_version", version)
        .map_err(|e| PebbleError::Storage(format!("Failed to set schema version: {e}")))
}

fn is_valid_account_color(color: &str) -> bool {
    color.len() == 7
        && color.as_bytes()[0] == b'#'
        && color.as_bytes()[1..].iter().all(|b| b.is_ascii_hexdigit())
}

fn derive_account_color(seed: &str) -> String {
    let mut hash = 0u32;
    for byte in seed.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u32);
    }
    ACCOUNT_COLOR_PRESETS[(hash as usize) % ACCOUNT_COLOR_PRESETS.len()].to_string()
}

fn backfill_account_colors(conn: &Connection) -> Result<()> {
    let accounts = {
        let mut stmt =
            conn.prepare("SELECT id, color FROM accounts ORDER BY created_at ASC, id ASC")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })?;
        let mut accounts = Vec::new();
        for row in rows {
            accounts.push(row?);
        }
        accounts
    };

    let mut used_colors: HashSet<String> = accounts
        .iter()
        .filter_map(|(_, color)| color.as_deref())
        .filter(|color| is_valid_account_color(color))
        .map(str::to_ascii_lowercase)
        .collect();

    for (id, color) in accounts {
        if color.as_deref().is_some_and(is_valid_account_color) {
            continue;
        }

        let selected = ACCOUNT_COLOR_PRESETS
            .iter()
            .find(|candidate| !used_colors.contains(**candidate))
            .map(|color| (*color).to_string())
            .unwrap_or_else(|| derive_account_color(&id));
        used_colors.insert(selected.clone());
        conn.execute(
            "UPDATE accounts SET color = ?1 WHERE id = ?2",
            rusqlite::params![selected, id],
        )?;
    }

    Ok(())
}

fn accounts_provider_check_allows_pop3(conn: &Connection) -> Result<bool> {
    let sql: String = conn.query_row(
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'accounts'",
        [],
        |row| row.get(0),
    )?;
    Ok(sql.contains("'pop3'"))
}

fn rebuild_accounts_with_pop3_provider(conn: &Connection) -> Result<()> {
    if conn
        .prepare("SELECT auth_data FROM accounts LIMIT 0")
        .is_err()
    {
        conn.execute_batch("ALTER TABLE accounts ADD COLUMN auth_data BLOB;")
            .map_err(|e| {
                PebbleError::Storage(format!("Migration V12 auth_data column failed: {e}"))
            })?;
    }
    if conn
        .prepare("SELECT sync_state FROM accounts LIMIT 0")
        .is_err()
    {
        conn.execute_batch("ALTER TABLE accounts ADD COLUMN sync_state TEXT;")
            .map_err(|e| {
                PebbleError::Storage(format!("Migration V12 sync_state column failed: {e}"))
            })?;
    }

    conn.execute_batch(
        "CREATE TABLE accounts_new (
            id TEXT PRIMARY KEY,
            email TEXT NOT NULL,
            display_name TEXT NOT NULL DEFAULT '',
            color TEXT,
            provider TEXT NOT NULL CHECK(provider IN ('imap', 'pop3', 'gmail', 'outlook')),
            auth_data BLOB,
            sync_state TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );
        INSERT INTO accounts_new (id, email, display_name, color, provider, auth_data, sync_state, created_at, updated_at)
            SELECT id, email, display_name, color, provider, auth_data, sync_state, created_at, updated_at
            FROM accounts;
        DROP TABLE accounts;
        ALTER TABLE accounts_new RENAME TO accounts;",
    )
    .map_err(|e| PebbleError::Storage(format!("Migration V12 failed: {e}")))?;
    Ok(())
}

fn rebuild_snippets(conn: &Connection) -> Result<()> {
    let mut stmt = match conn.prepare("SELECT id, body_text, body_html_raw FROM messages") {
        Ok(s) => s,
        Err(_) => return Ok(()),
    };
    let rows: Vec<(String, String, String)> = stmt
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .map_err(|e| PebbleError::Storage(format!("V13 query failed: {e}")))?
        .filter_map(|r| r.ok())
        .collect();

    let mut update = conn
        .prepare("UPDATE messages SET snippet = ?1 WHERE id = ?2")
        .map_err(|e| PebbleError::Storage(format!("V13 prepare update failed: {e}")))?;
    for (id, body_text, body_html) in &rows {
        let new_snippet = build_snippet(body_text, body_html);
        update
            .execute(rusqlite::params![new_snippet, id])
            .map_err(|e| PebbleError::Storage(format!("V13 update failed: {e}")))?;
    }
    Ok(())
}

pub fn run_migrations(conn: &Connection) -> Result<()> {
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    conn.execute_batch("PRAGMA synchronous=NORMAL;")?;

    conn.execute_batch("PRAGMA foreign_keys=ON;")?;

    conn.execute_batch("PRAGMA busy_timeout=5000;")?;

    let version = get_schema_version(conn);

    // Each migration is wrapped in a transaction so that the DDL and version
    // update are atomic; a crash mid-migration won't leave an inconsistent state.

    if version < 1 {
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| PebbleError::Storage(format!("Migration V1 begin failed: {e}")))?;
        tx.execute_batch(SCHEMA_V1)
            .map_err(|e| PebbleError::Storage(format!("Migration V1 failed: {e}")))?;
        set_schema_version(&tx, 1)?;
        tx.commit()
            .map_err(|e| PebbleError::Storage(format!("Migration V1 commit failed: {e}")))?;
    }

    if version < 2 {
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| PebbleError::Storage(format!("Migration V2 begin failed: {e}")))?;
        let has_content_id: bool = tx
            .prepare("SELECT content_id FROM attachments LIMIT 0")
            .is_ok();
        if !has_content_id {
            tx.execute_batch(
                "ALTER TABLE attachments ADD COLUMN content_id TEXT;
                 ALTER TABLE attachments ADD COLUMN is_inline INTEGER NOT NULL DEFAULT 0;",
            )
            .map_err(|e| PebbleError::Storage(format!("Migration V2 failed: {e}")))?;
        }
        set_schema_version(&tx, 2)?;
        tx.commit()
            .map_err(|e| PebbleError::Storage(format!("Migration V2 commit failed: {e}")))?;
    }

    if version < 3 {
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| PebbleError::Storage(format!("Migration V3 begin failed: {e}")))?;
        tx.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_messages_account_remote ON messages(account_id, remote_id);
             CREATE INDEX IF NOT EXISTS idx_snoozed_unsnoozed_at ON snoozed_messages(unsnoozed_at);
             CREATE UNIQUE INDEX IF NOT EXISTS idx_folders_account_remote ON folders(account_id, remote_id);"
        )
        .map_err(|e| PebbleError::Storage(format!("Migration V3 failed: {e}")))?;
        set_schema_version(&tx, 3)?;
        tx.commit()
            .map_err(|e| PebbleError::Storage(format!("Migration V3 commit failed: {e}")))?;
    }

    if version < 4 {
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| PebbleError::Storage(format!("Migration V4 begin failed: {e}")))?;
        tx.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_message_folders_folder_id ON message_folders(folder_id);
             CREATE INDEX IF NOT EXISTS idx_messages_account_starred ON messages(account_id, is_starred) WHERE is_starred = 1 AND is_deleted = 0;
             CREATE INDEX IF NOT EXISTS idx_messages_thread_date ON messages(thread_id, date) WHERE thread_id IS NOT NULL AND is_deleted = 0;"
        )
        .map_err(|e| PebbleError::Storage(format!("Migration V4 failed: {e}")))?;
        set_schema_version(&tx, 4)?;
        tx.commit()
            .map_err(|e| PebbleError::Storage(format!("Migration V4 commit failed: {e}")))?;
    }

    if version < 5 {
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| PebbleError::Storage(format!("Migration V5 begin failed: {e}")))?;
        tx.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_mf_folder_message ON message_folders(folder_id, message_id);",
        )
        .map_err(|e| PebbleError::Storage(format!("Migration V5 failed: {e}")))?;
        set_schema_version(&tx, 5)?;
        tx.commit()
            .map_err(|e| PebbleError::Storage(format!("Migration V5 commit failed: {e}")))?;
    }

    // V6: search_pending table for crash-recovery of the search index
    if version < 6 {
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| PebbleError::Storage(format!("Migration V6 begin failed: {e}")))?;
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS search_pending (
                 message_id TEXT PRIMARY KEY,
                 operation TEXT NOT NULL CHECK(operation IN ('index', 'remove')),
                 created_at INTEGER NOT NULL
             );",
        )
        .map_err(|e| PebbleError::Storage(format!("Migration V6 failed: {e}")))?;
        set_schema_version(&tx, 6)?;
        tx.commit()
            .map_err(|e| PebbleError::Storage(format!("Migration V6 commit failed: {e}")))?;
    }

    if version < 7 {
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| PebbleError::Storage(format!("Migration V7 begin failed: {e}")))?;
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS folder_sync_state (
                 account_id TEXT NOT NULL,
                 folder_id TEXT NOT NULL,
                 state TEXT NOT NULL,
                 updated_at INTEGER NOT NULL,
                 PRIMARY KEY (account_id, folder_id),
                 FOREIGN KEY(account_id) REFERENCES accounts(id) ON DELETE CASCADE,
                 FOREIGN KEY(folder_id) REFERENCES folders(id) ON DELETE CASCADE
             );",
        )
        .map_err(|e| PebbleError::Storage(format!("Migration V7 failed: {e}")))?;
        set_schema_version(&tx, 7)?;
        tx.commit()
            .map_err(|e| PebbleError::Storage(format!("Migration V7 commit failed: {e}")))?;
    }

    if version < 8 {
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| PebbleError::Storage(format!("Migration V8 begin failed: {e}")))?;
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS sync_failures (
                 account_id TEXT NOT NULL,
                 folder_id TEXT NOT NULL,
                 remote_id TEXT NOT NULL,
                 provider TEXT NOT NULL,
                 reason TEXT NOT NULL,
                 attempts INTEGER NOT NULL DEFAULT 1,
                 updated_at INTEGER NOT NULL,
                 PRIMARY KEY (account_id, folder_id, remote_id),
                 FOREIGN KEY(account_id) REFERENCES accounts(id) ON DELETE CASCADE,
                 FOREIGN KEY(folder_id) REFERENCES folders(id) ON DELETE CASCADE
             );
             CREATE INDEX IF NOT EXISTS idx_sync_failures_folder
                 ON sync_failures(account_id, folder_id);",
        )
        .map_err(|e| PebbleError::Storage(format!("Migration V8 failed: {e}")))?;
        set_schema_version(&tx, 8)?;
        tx.commit()
            .map_err(|e| PebbleError::Storage(format!("Migration V8 commit failed: {e}")))?;
    }

    if version < 9 {
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| PebbleError::Storage(format!("Migration V9 begin failed: {e}")))?;
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS pending_mail_ops (
                 id TEXT PRIMARY KEY,
                 account_id TEXT NOT NULL,
                 message_id TEXT NOT NULL,
                 op_type TEXT NOT NULL,
                 payload_json TEXT NOT NULL,
                 status TEXT NOT NULL CHECK(status IN ('pending', 'in_progress', 'failed', 'done')),
                 attempts INTEGER NOT NULL DEFAULT 0,
                 last_error TEXT,
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_pending_mail_ops_account_status
                 ON pending_mail_ops(account_id, status, updated_at);",
        )
        .map_err(|e| PebbleError::Storage(format!("Migration V9 failed: {e}")))?;
        set_schema_version(&tx, 9)?;
        tx.commit()
            .map_err(|e| PebbleError::Storage(format!("Migration V9 commit failed: {e}")))?;
    }

    if version < 10 {
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| PebbleError::Storage(format!("Migration V10 begin failed: {e}")))?;
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS secure_user_data (
                 key TEXT PRIMARY KEY,
                 value BLOB NOT NULL,
                 updated_at INTEGER NOT NULL
             );
             ALTER TABLE pending_mail_ops ADD COLUMN next_retry_at INTEGER;
             CREATE INDEX IF NOT EXISTS idx_pending_mail_ops_retry
                 ON pending_mail_ops(status, next_retry_at, updated_at);",
        )
        .map_err(|e| PebbleError::Storage(format!("Migration V10 failed: {e}")))?;
        set_schema_version(&tx, 10)?;
        tx.commit()
            .map_err(|e| PebbleError::Storage(format!("Migration V10 commit failed: {e}")))?;
    }

    if version < 11 {
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| PebbleError::Storage(format!("Migration V11 begin failed: {e}")))?;
        let has_color: bool = tx.prepare("SELECT color FROM accounts LIMIT 0").is_ok();
        if !has_color {
            tx.execute_batch("ALTER TABLE accounts ADD COLUMN color TEXT;")
                .map_err(|e| PebbleError::Storage(format!("Migration V11 failed: {e}")))?;
        }
        backfill_account_colors(&tx)?;
        set_schema_version(&tx, CURRENT_VERSION)?;
        tx.commit()
            .map_err(|e| PebbleError::Storage(format!("Migration V11 commit failed: {e}")))?;
    }

    if version < 12 {
        conn.execute_batch("PRAGMA foreign_keys=OFF;")
            .map_err(|e| PebbleError::Storage(format!("Migration V12 disable FK failed: {e}")))?;
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| PebbleError::Storage(format!("Migration V12 begin failed: {e}")))?;
        if !accounts_provider_check_allows_pop3(&tx)? {
            rebuild_accounts_with_pop3_provider(&tx)?;
        }
        set_schema_version(&tx, CURRENT_VERSION)?;
        tx.commit()
            .map_err(|e| PebbleError::Storage(format!("Migration V12 commit failed: {e}")))?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")
            .map_err(|e| PebbleError::Storage(format!("Migration V12 enable FK failed: {e}")))?;
        let fk_violations: i64 = conn
            .query_row("PRAGMA foreign_key_check", [], |_| Ok(1))
            .optional()
            .map_err(|e| PebbleError::Storage(format!("Migration V12 FK check failed: {e}")))?
            .unwrap_or(0);
        if fk_violations != 0 {
            return Err(PebbleError::Storage(
                "Migration V12 introduced foreign key violations".to_string(),
            ));
        }
    }

    // V13: rebuild snippets to strip leaked HTML/CSS from previews
    if version < 13 {
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| PebbleError::Storage(format!("Migration V13 begin failed: {e}")))?;
        rebuild_snippets(&tx)?;
        set_schema_version(&tx, CURRENT_VERSION)?;
        tx.commit()
            .map_err(|e| PebbleError::Storage(format!("Migration V13 commit failed: {e}")))?;
    }

    Ok(())
}

const SCHEMA_V1: &str = r#"
CREATE TABLE IF NOT EXISTS accounts (
    id TEXT PRIMARY KEY,
    email TEXT NOT NULL,
    display_name TEXT NOT NULL DEFAULT '',
    color TEXT,
    provider TEXT NOT NULL CHECK(provider IN ('imap', 'pop3', 'gmail', 'outlook')),
    auth_data BLOB,
    sync_state TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS folders (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    remote_id TEXT NOT NULL,
    name TEXT NOT NULL,
    folder_type TEXT NOT NULL CHECK(folder_type IN ('folder', 'label', 'category')),
    role TEXT CHECK(role IN ('inbox', 'sent', 'drafts', 'trash', 'archive', 'spam')),
    parent_id TEXT,
    color TEXT,
    is_system INTEGER NOT NULL DEFAULT 0,
    sort_order INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_folders_account ON folders(account_id);

CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    remote_id TEXT NOT NULL,
    message_id_header TEXT,
    in_reply_to TEXT,
    references_header TEXT,
    thread_id TEXT,
    subject TEXT NOT NULL DEFAULT '',
    snippet TEXT NOT NULL DEFAULT '',
    from_address TEXT NOT NULL DEFAULT '',
    from_name TEXT NOT NULL DEFAULT '',
    to_list TEXT NOT NULL DEFAULT '[]',
    cc_list TEXT NOT NULL DEFAULT '[]',
    bcc_list TEXT NOT NULL DEFAULT '[]',
    body_text TEXT NOT NULL DEFAULT '',
    body_html_raw TEXT NOT NULL DEFAULT '',
    has_attachments INTEGER NOT NULL DEFAULT 0,
    is_read INTEGER NOT NULL DEFAULT 0,
    is_starred INTEGER NOT NULL DEFAULT 0,
    is_draft INTEGER NOT NULL DEFAULT 0,
    date INTEGER NOT NULL,
    raw_headers TEXT,
    remote_version TEXT,
    is_deleted INTEGER NOT NULL DEFAULT 0,
    deleted_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_messages_account ON messages(account_id);
CREATE INDEX IF NOT EXISTS idx_messages_thread ON messages(thread_id);
CREATE INDEX IF NOT EXISTS idx_messages_date ON messages(date);
CREATE INDEX IF NOT EXISTS idx_messages_message_id_header ON messages(message_id_header);

CREATE TABLE IF NOT EXISTS message_folders (
    message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    folder_id TEXT NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
    PRIMARY KEY (message_id, folder_id)
);

CREATE TABLE IF NOT EXISTS attachments (
    id TEXT PRIMARY KEY,
    message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    filename TEXT NOT NULL DEFAULT '',
    mime_type TEXT NOT NULL DEFAULT '',
    size INTEGER NOT NULL DEFAULT 0,
    local_path TEXT,
    content_id TEXT,
    is_inline INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_attachments_message ON attachments(message_id);

CREATE TABLE IF NOT EXISTS labels (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    color TEXT NOT NULL DEFAULT '#808080',
    is_system INTEGER NOT NULL DEFAULT 0,
    rule_id TEXT
);

CREATE TABLE IF NOT EXISTS message_labels (
    message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    label_id TEXT NOT NULL REFERENCES labels(id) ON DELETE CASCADE,
    PRIMARY KEY (message_id, label_id)
);

CREATE TABLE IF NOT EXISTS kanban_cards (
    message_id TEXT PRIMARY KEY REFERENCES messages(id) ON DELETE CASCADE,
    column_name TEXT NOT NULL CHECK(column_name IN ('todo', 'waiting', 'done')),
    position INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS snoozed_messages (
    message_id TEXT PRIMARY KEY REFERENCES messages(id) ON DELETE CASCADE,
    snoozed_at INTEGER NOT NULL,
    unsnoozed_at INTEGER NOT NULL,
    return_to TEXT NOT NULL DEFAULT 'inbox'
);

CREATE TABLE IF NOT EXISTS trusted_senders (
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    email TEXT NOT NULL,
    trust_type TEXT NOT NULL CHECK(trust_type IN ('images', 'all')),
    created_at INTEGER NOT NULL,
    PRIMARY KEY (account_id, email)
);

CREATE TABLE IF NOT EXISTS rules (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    priority INTEGER NOT NULL DEFAULT 0,
    conditions TEXT NOT NULL DEFAULT '{}',
    actions TEXT NOT NULL DEFAULT '[]',
    is_enabled INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS translate_config (
    id TEXT PRIMARY KEY DEFAULT 'active',
    provider_type TEXT NOT NULL CHECK(provider_type IN ('deeplx', 'deepl', 'generic_api', 'llm')),
    config TEXT NOT NULL DEFAULT '{}',
    is_enabled INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_v11_adds_account_color_and_sets_schema_version() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE accounts (
                id TEXT PRIMARY KEY,
                email TEXT NOT NULL,
                display_name TEXT NOT NULL DEFAULT '',
                provider TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            PRAGMA user_version = 10;",
        )
        .unwrap();

        run_migrations(&conn).unwrap();

        let version: u32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, CURRENT_VERSION);
        conn.prepare("SELECT color FROM accounts LIMIT 0")
            .expect("accounts.color should exist after V11");
    }

    #[test]
    fn migration_v11_backfills_existing_account_colors() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE accounts (
                id TEXT PRIMARY KEY,
                email TEXT NOT NULL,
                display_name TEXT NOT NULL DEFAULT '',
                provider TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            INSERT INTO accounts (id, email, display_name, provider, created_at, updated_at)
            VALUES
                ('account-1', 'one@example.com', 'One', 'gmail', 1, 1),
                ('account-2', 'two@example.com', 'Two', 'gmail', 2, 2);
            PRAGMA user_version = 10;",
        )
        .unwrap();

        run_migrations(&conn).unwrap();

        let mut stmt = conn
            .prepare("SELECT color FROM accounts ORDER BY created_at ASC")
            .unwrap();
        let colors = stmt
            .query_map([], |row| row.get::<_, Option<String>>(0))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            colors,
            vec![Some("#0ea5e9".to_string()), Some("#22c55e".to_string())]
        );
    }

    #[test]
    fn migration_v12_allows_pop3_provider_without_breaking_foreign_keys() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys=ON;
            CREATE TABLE accounts (
                id TEXT PRIMARY KEY,
                email TEXT NOT NULL,
                display_name TEXT NOT NULL DEFAULT '',
                color TEXT,
                provider TEXT NOT NULL CHECK(provider IN ('imap', 'gmail', 'outlook')),
                auth_data BLOB,
                sync_state TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            CREATE TABLE folders (
                id TEXT PRIMARY KEY,
                account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
                remote_id TEXT NOT NULL,
                name TEXT NOT NULL,
                folder_type TEXT NOT NULL CHECK(folder_type IN ('folder', 'label', 'category')),
                role TEXT CHECK(role IN ('inbox', 'sent', 'drafts', 'trash', 'archive', 'spam')),
                parent_id TEXT,
                color TEXT,
                is_system INTEGER NOT NULL DEFAULT 0,
                sort_order INTEGER NOT NULL DEFAULT 0
            );
            INSERT INTO accounts (id, email, display_name, color, provider, created_at, updated_at)
                VALUES ('account-1', 'one@example.com', 'One', '#0ea5e9', 'imap', 1, 1);
            INSERT INTO folders (id, account_id, remote_id, name, folder_type, role, is_system, sort_order)
                VALUES ('folder-1', 'account-1', 'INBOX', 'Inbox', 'folder', 'inbox', 1, 0);
            PRAGMA user_version = 11;",
        )
        .unwrap();

        run_migrations(&conn).unwrap();

        let version: u32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, CURRENT_VERSION);
        conn.execute(
            "INSERT INTO accounts (id, email, display_name, provider, created_at, updated_at)
                VALUES ('account-2', 'two@example.com', 'Two', 'pop3', 2, 2)",
            [],
        )
        .expect("accounts.provider should accept pop3 after V12");
        let folder_account: String = conn
            .query_row(
                "SELECT account_id FROM folders WHERE id = 'folder-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(folder_account, "account-1");
        let fk_issue = conn
            .query_row("PRAGMA foreign_key_check", [], |_| Ok(()))
            .optional()
            .unwrap();
        assert!(fk_issue.is_none());
    }
}
