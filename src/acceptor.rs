use super::*;
use crate::config::{AccessState, AuthState};
use crate::logger::{log_event, log_failure};

pub struct Socks5Acceptor {
    pub buf: Vec<u8>,
    pub stream: TcpStream,
    pub auth: Option<AuthState>,
    pub access: Option<AccessState>,
    pub username: Option<String>,
}

impl Socks5Acceptor {
    pub fn allow_domains(&self) -> bool {
        self.access
            .as_ref()
            .map(|x| x.allow_domains())
            .unwrap_or(false)
    }

    pub async fn authenticate(&mut self) -> Result<()> {
        let client_addr = self.peer_addr();
        let client_addr_str = client_addr.to_string();
        let client_ip = client_addr.ip().to_string();
        self.buf.resize(2, 0);
        self.stream.read_exact(&mut self.buf).await?;

        if self.buf[0] != 5 {
            log_failure(
                "auth_failed",
                self.username.as_deref(),
                &client_addr_str,
                &client_ip,
                None,
                "not_socks5_request",
            );
            return Err("Not socks5 request!".into());
        }

        self.buf.resize(2 + self.buf[1] as usize, 0);
        self.stream.read_exact(&mut self.buf[2..]).await?;

        match &self.auth {
            Some(_) => {
                if !self.buf[2..].contains(&0x02) {
                    self.stream.write_all(b"\x05\xff").await?;
                    log_failure(
                        "auth_failed",
                        self.username.as_deref(),
                        &client_addr_str,
                        &client_ip,
                        None,
                        "no_supported_auth_method",
                    );
                    return Err("No supported authentication method!".into());
                }
                self.stream.write_all(b"\x05\x02").await?;
                self.authenticate_userpass().await?;
            }
            None => {
                if !self.buf[2..].contains(&0x00) {
                    self.stream.write_all(b"\x05\xff").await?;
                    log_failure(
                        "auth_failed",
                        self.username.as_deref(),
                        &client_addr_str,
                        &client_ip,
                        None,
                        "no_supported_auth_method",
                    );
                    return Err("No supported authentication method!".into());
                }
                self.stream.write_all(b"\x05\x00").await?;
                log_event(
                    "auth_succeeded",
                    self.username.as_deref(),
                    &client_addr_str,
                    &client_ip,
                    None,
                );
            }
        }
        Ok(())
    }

    async fn authenticate_userpass(&mut self) -> Result<()> {
        let client_addr = self.peer_addr();
        let client_addr_str = client_addr.to_string();
        let client_ip = client_addr.ip().to_string();
        self.buf.resize(2, 0);
        self.stream.read_exact(&mut self.buf).await?;
        if self.buf[0] != 0x01 {
            self.stream.write_all(&[0x01, 0x01]).await?;
            log_failure(
                "auth_failed",
                self.username.as_deref(),
                &client_addr_str,
                &client_ip,
                None,
                "invalid_userpass_auth_version",
            );
            return Err("Invalid username/password auth version!".into());
        }

        let ulen = self.buf[1] as usize;
        self.buf.resize(2 + ulen + 1, 0);
        self.stream
            .read_exact(&mut self.buf[2..(2 + ulen + 1)])
            .await?;
        let plen = self.buf[2 + ulen] as usize;
        self.buf.resize(2 + ulen + 1 + plen, 0);
        self.stream
            .read_exact(&mut self.buf[(2 + ulen + 1)..])
            .await?;

        let username = std::str::from_utf8(&self.buf[2..2 + ulen])
            .map_err(|_| Error::from("Invalid UTF-8 username!"))?;
        let password = std::str::from_utf8(&self.buf[(2 + ulen + 1)..(2 + ulen + 1 + plen)])
            .map_err(|_| Error::from("Invalid UTF-8 password!"))?;

        let ok = self
            .auth
            .as_ref()
            .map(|x| x.verify(username, password))
            .unwrap_or(false);
        if ok {
            self.username = Some(username.to_owned());
            self.stream.write_all(&[0x01, 0x00]).await?;
            log_event(
                "auth_succeeded",
                self.username.as_deref(),
                &client_addr_str,
                &client_ip,
                None,
            );
            Ok(())
        } else {
            self.stream.write_all(&[0x01, 0x01]).await?;
            log_failure(
                "auth_failed",
                Some(username),
                &client_addr_str,
                &client_ip,
                None,
                "invalid_username_or_password",
            );
            Err("Username/password authentication failed!".into())
        }
    }

    pub async fn accept(mut self) -> Result<()> {
        self.authenticate().await?;
        let (command, target) = self.accept_command().await?;
        let target = Socks5Target::try_from(target)?;

        if command == 3 {
            self.associate_udp(target).await
        } else {
            self.connect(target).await
        }
    }

    pub async fn accept_command(&mut self) -> Result<(u8, &[u8])> {
        self.buf.resize(5, 0);
        self.stream.read_exact(&mut self.buf).await?;

        if self.buf[0] != 5 || self.buf[2] != 0 {
            return Err("Invalid request!".into());
        }

        let len = match Socks5Target::target_len(&self.buf[3..]) {
            Ok(x) => x + 3,
            Err(e) => {
                self.stream.write_all(b"\x05\x08").await?;
                return Err(e);
            }
        };

        self.buf.resize(len, 0);
        self.stream.read_exact(&mut self.buf[5..]).await?;

        if self.buf[1] != 1 && self.buf[1] != 3 {
            self.stream.write_all(b"\x05\x07").await?;
            return Err("Unsupported request command!".into());
        }

        Ok((self.buf[1], &self.buf[3..]))
    }

    pub async fn connected(&mut self, local_addr: SocketAddr) -> Result<()> {
        let mut reply = b"\x05\x00\x00".to_vec();
        reply.put_socks5_addr(local_addr);
        self.stream.write_all(&reply).await?;
        Ok(())
    }

    pub async fn closed(mut self, resp: u8) -> Result<()> {
        // resp:
        //   0x00 succeeded
        //   0x01 general SOCKS server failure
        //   0x02 connection not allowed by ruleset
        //   0x03 Network unreachable
        //   0x04 Host unreachable
        //   0x05 Connection refused
        //   0x06 TTL expired
        //   0x07 Command not supported
        //   0x08 Address type not supported
        //   0x09 to 0xff unassigned
        let mut reply = vec![0x05, resp, 0x00];
        reply.extend_from_slice(&self.buf[3..]);
        self.stream.write_all(&reply).await?;
        Ok(())
    }

    pub fn peer_addr(&self) -> SocketAddr {
        self.stream.peer_addr().unwrap()
    }
}

impl From<TcpStream> for Socks5Acceptor {
    fn from(stream: TcpStream) -> Self {
        Self {
            stream,
            buf: Vec::with_capacity(64),
            auth: None,
            access: None,
            username: None,
        }
    }
}
