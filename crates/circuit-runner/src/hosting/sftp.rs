//! Uploading over SFTP (SSH). The server's key is trusted the first time and
//! kept (in `known_hosts`, a JSON file of "host:port" → fingerprint); a server
//! that later shows another key is refused, as it may not be the same server.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use russh::client;
use russh::keys::{HashAlg, PrivateKeyWithHashAlg};
use russh_sftp::client::SftpSession;
use tokio::io::AsyncWriteExt;

use super::{Hosting, Remote};

pub(super) struct Sftp {
    runtime: tokio::runtime::Runtime,
    session: SftpSession,
    ssh: client::Handle<Keys>,
    /// The bot's folder, ending in "/" (empty: the login's own folder).
    base: String,
}

/// Checks the server's key against the one kept for it.
struct Keys {
    place: String,
    known_hosts: PathBuf,
    /// What happened: the key was new (and is now trusted), or it changed.
    note: Arc<Mutex<Option<KeyNote>>>,
}

enum KeyNote {
    New(String),
    Changed { was: String, now: String },
}

impl client::Handler for Keys {
    type Error = russh::Error;

    async fn check_server_key(&mut self, key: &russh::keys::PublicKeyOrCertificate) -> Result<bool, Self::Error> {
        let now = key.public_key().fingerprint(HashAlg::Sha256).to_string();
        let mut known = read_known(&self.known_hosts);
        match known.get(&self.place) {
            Some(was) if *was == now => Ok(true),
            Some(was) => {
                *self.note.lock().unwrap() = Some(KeyNote::Changed { was: was.clone(), now });
                Ok(false)
            }
            None => {
                known.insert(self.place.clone(), now.clone());
                let _ = write_known(&self.known_hosts, &known);
                *self.note.lock().unwrap() = Some(KeyNote::New(now));
                Ok(true)
            }
        }
    }
}

fn read_known(path: &Path) -> BTreeMap<String, String> {
    std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn write_known(path: &Path, known: &BTreeMap<String, String>) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(known)?)?;
    Ok(())
}

/// Forgets the key kept for a server, so the next connection trusts the one
/// it shows (after the server really got a new one).
pub fn forget_key(known_hosts: &Path, host: &str, port: u16) -> Result<()> {
    let mut known = read_known(known_hosts);
    known.remove(&format!("{host}:{port}"));
    write_known(known_hosts, &known)
}

impl Sftp {
    pub(super) fn open(h: &Hosting, password: Option<&str>, known_hosts: &Path, log: &mut dyn FnMut(String)) -> Result<Sftp> {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
        let host = h.host.trim().to_string();
        let port = h.port();
        let note = Arc::new(Mutex::new(None));
        let keys = Keys { place: format!("{host}:{port}"), known_hosts: known_hosts.to_path_buf(), note: note.clone() };
        let config = Arc::new(client::Config { inactivity_timeout: Some(Duration::from_secs(60)), ..Default::default() });
        log(format!("Connecting to {host}:{port} (SFTP) …"));
        let connected = runtime.block_on(async { tokio::time::timeout(Duration::from_secs(20), client::connect(config, (host.as_str(), port), keys)).await });
        let note = note.lock().unwrap().take();
        if let Some(KeyNote::Changed { was, now }) = &note {
            bail!(
                "{host} shows another key than before ({now}, it was {was}). That can mean it isn't your server. If the server really got a new key, choose Forget the server's key and upload again."
            );
        }
        let mut ssh = connected.map_err(|_| anyhow!("{host}:{port} didn't answer within 20 seconds"))?.with_context(|| format!("couldn't connect to {host}:{port}"))?;
        if let Some(KeyNote::New(key)) = note {
            log(format!("First connection to {host}: its key {key} is kept, to make sure it's the same server next time."));
        }
        let user = h.user.trim().to_string();
        let base = runtime.block_on(async {
            let accepted = if h.key_file.trim().is_empty() {
                ssh.authenticate_password(user.clone(), password.unwrap_or_default()).await?.success()
            } else {
                let key = russh::keys::load_secret_key(h.key_file.trim(), password).map_err(|e| anyhow!("couldn't read the key file {}: {e}", h.key_file.trim()))?;
                let hash = ssh.best_supported_rsa_hash().await?.flatten();
                ssh.authenticate_publickey(user.clone(), PrivateKeyWithHashAlg::new(Arc::new(key), hash)).await?.success()
            };
            if !accepted {
                bail!("{host} didn't accept the user name and {}", if h.key_file.trim().is_empty() { "password" } else { "key" });
            }
            Ok::<_, anyhow::Error>(())
        });
        base?;
        let session = runtime.block_on(async {
            let channel = ssh.channel_open_session().await?;
            channel.request_subsystem(true, "sftp").await?;
            SftpSession::new(channel.into_stream()).await.map_err(|e| anyhow!("{host} doesn't offer SFTP: {e}"))
        })?;
        // The folder, made where it's missing.
        let (absolute, parts) = h.folder_parts();
        let mut path = if absolute { "/".to_string() } else { String::new() };
        runtime.block_on(async {
            for part in &parts {
                path.push_str(part);
                if !session.try_exists(path.clone()).await.unwrap_or(false) {
                    session.create_dir(path.clone()).await.map_err(|e| anyhow!("couldn't make the folder {path}: {e}"))?;
                }
                path.push('/');
            }
            Ok::<_, anyhow::Error>(())
        })?;
        log(format!("Logged in to {host} as {user}."));
        Ok(Sftp { runtime, session, ssh, base: path })
    }
}

impl Remote for Sftp {
    fn put(&mut self, name: &str, data: &[u8]) -> Result<()> {
        let path = format!("{}{name}", self.base);
        self.runtime.block_on(async {
            let mut file = self.session.create(path.clone()).await.map_err(|e| anyhow!("couldn't write {path}: {e}"))?;
            file.write_all(data).await?;
            file.shutdown().await?;
            Ok(())
        })
    }

    fn finish(&mut self, _: &mut dyn FnMut(String)) -> Result<()> {
        self.runtime.block_on(async {
            let _ = self.session.close().await;
            let _ = self.ssh.disconnect(russh::Disconnect::ByApplication, "", "en").await;
        });
        Ok(())
    }
}
