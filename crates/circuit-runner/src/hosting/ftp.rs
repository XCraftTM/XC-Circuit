//! Uploading over FTP, encrypted (FTPS, the usual "explicit" kind) unless
//! turned off.

use std::io::Cursor;
use std::net::ToSocketAddrs;
use std::time::Duration;

use anyhow::{Result, anyhow};
use suppaftp::types::FileType;
use suppaftp::{RustlsConnector, RustlsFtpStream};

use super::{Hosting, Remote};

pub(super) struct Ftp {
    ftp: RustlsFtpStream,
}

impl Ftp {
    pub(super) fn open(h: &Hosting, password: &str, log: &mut dyn FnMut(String)) -> Result<Ftp> {
        let host = h.host.trim();
        let port = h.port();
        let addr = (host, port).to_socket_addrs().map_err(|e| anyhow!("couldn't find {host}: {e}"))?.next().ok_or_else(|| anyhow!("couldn't find {host}"))?;
        log(format!("Connecting to {host}:{port} ({}) …", if h.ftps { "FTPS" } else { "FTP" }));
        let mut ftp = RustlsFtpStream::connect_timeout(addr, Duration::from_secs(20)).map_err(|e| anyhow!("couldn't connect to {host}:{port}: {e}"))?;
        if h.ftps {
            ftp = ftp
                .into_secure(RustlsConnector::from(super::tls_config()), host)
                .map_err(|e| anyhow!("{host} couldn't make an encrypted connection ({e}). If it only does plain FTP, turn off Encrypted (FTPS)."))?;
        }
        ftp.login(h.user.trim(), password).map_err(|e| anyhow!("{host} didn't accept the user name and password: {e}"))?;
        ftp.transfer_type(FileType::Binary)?;
        // The folder, made where it's missing.
        let (absolute, parts) = h.folder_parts();
        if absolute {
            ftp.cwd("/")?;
        }
        for part in parts {
            if ftp.cwd(&part).is_err() {
                ftp.mkdir(&part).map_err(|e| anyhow!("couldn't make the folder {part}: {e}"))?;
                ftp.cwd(&part)?;
            }
        }
        log(format!("Logged in to {host} as {}.", h.user.trim()));
        Ok(Ftp { ftp })
    }
}

impl Remote for Ftp {
    fn put(&mut self, name: &str, data: &[u8]) -> Result<()> {
        self.ftp.put_file(name, &mut Cursor::new(data)).map_err(|e| anyhow!("{e}"))?;
        Ok(())
    }

    fn finish(&mut self, _: &mut dyn FnMut(String)) -> Result<()> {
        let _ = self.ftp.quit();
        Ok(())
    }
}
