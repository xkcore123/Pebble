//! Local, credential-free IMAP fixtures for connection recovery regressions.
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::task::{JoinHandle, JoinSet};

use crate::imap::{ConnectionSecurity, ImapConfig};

#[derive(Clone, Copy)]
pub(crate) enum Behavior {
    SelectDenied,
    MissingInbox,
    ByeThenEmpty,
    ByeThenDenied,
    SilentDone,
    SilentLogout,
    DropFirstLogin,
    RejectLogin,
}

pub(crate) struct Server {
    port: u16,
    commands: Arc<Mutex<Vec<String>>>,
    task: JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Server {
    pub(crate) async fn start(behavior: Behavior) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let commands = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&commands);
        let task = tokio::spawn(async move {
            let mut clients = JoinSet::new();
            let mut connection = 0;
            loop {
                let (socket, _) = listener.accept().await.unwrap();
                let index = connection;
                connection += 1;
                let recorded = Arc::clone(&recorded);
                clients.spawn(async move {
                    let mut stream = BufReader::new(socket);
                    stream.get_mut().write_all(b"* OK Test IMAP ready\r\n").await.unwrap();
                    let mut idle_tag = String::new();
                    loop {
                        let mut line = String::new();
                        if stream.read_line(&mut line).await.unwrap_or(0) == 0 {
                            break;
                        }
                        let mut words = line.split_whitespace();
                        let tag = words.next().unwrap().to_string();
                        let command = words.next().unwrap_or("").to_ascii_uppercase();
                        // Never record LOGIN arguments, even in test fixtures.
                        recorded.lock().unwrap().push(command.clone());
                        let reply = if tag == "DONE" {
                            if matches!(behavior, Behavior::SilentDone) {
                                std::future::pending::<()>().await;
                            }
                            format!("{idle_tag} OK IDLE completed\r\n")
                        } else if command == "LOGIN" {
                            if matches!(behavior, Behavior::DropFirstLogin) && index == 0 {
                                break;
                            }
                            if matches!(behavior, Behavior::RejectLogin) {
                                stream.get_mut().write_all(format!("{tag} NO Invalid credentials\r\n").as_bytes()).await.unwrap();
                                break;
                            }
                            format!("{tag} OK LOGIN completed\r\n")
                        } else if command == "SELECT" {
                            match behavior {
                                Behavior::ByeThenEmpty | Behavior::ByeThenDenied if index == 0 => {
                                    stream.get_mut().write_all(b"* BYE problems with connection\r\n").await.unwrap();
                                    break;
                                }
                                Behavior::SelectDenied | Behavior::ByeThenDenied => {
                                    format!("{tag} NO [NOPERM] Permission denied\r\n")
                                }
                                Behavior::MissingInbox => format!("{tag} NO SELECT Folder not exist\r\n"),
                                _ => format!("* FLAGS (\\Seen)\r\n* 0 EXISTS\r\n* OK [UIDVALIDITY 42] UIDs valid\r\n* OK [UIDNEXT 1] Next UID\r\n{tag} OK [READ-WRITE] SELECT completed\r\n"),
                            }
                        } else if command == "IDLE" {
                            idle_tag = tag;
                            "+ idling\r\n".to_string()
                        } else if command == "LOGOUT" {
                            if matches!(behavior, Behavior::SilentLogout) {
                                std::future::pending::<()>().await;
                            }
                            format!("* BYE logging out\r\n{tag} OK LOGOUT completed\r\n")
                        } else {
                            format!("{tag} OK completed\r\n")
                        };
                        if stream.get_mut().write_all(reply.as_bytes()).await.is_err() {
                            break;
                        }
                    }
                });
            }
        });
        Self {
            port,
            commands,
            task,
        }
    }

    pub(crate) fn config(&self) -> ImapConfig {
        ImapConfig {
            host: "127.0.0.1".to_string(),
            port: self.port,
            username: "fixture-user".to_string(),
            password: "fixture-password".to_string(),
            security: ConnectionSecurity::Plain,
            accept_invalid_certs: false,
            proxy: None,
        }
    }

    pub(crate) fn login_count(&self) -> usize {
        self.commands
            .lock()
            .unwrap()
            .iter()
            .filter(|cmd| cmd.as_str() == "LOGIN")
            .count()
    }
}
