use super::*;
use crate::config::AccessState;
use crate::logger::{log_access, log_event, log_failure};

pub struct Socks5TcpConnector(TcpStream);

impl Socks5TcpConnector {
    pub async fn connect(target: &Socks5Target, access: Option<&AccessState>) -> Result<Self> {
        let stream = match &target.0 {
            Socks5Host::IpAddr(x) => {
                Self::ensure_allowed(*x, access)?;
                TcpStream::connect((*x, target.1)).await?
            }
            Socks5Host::Domain(x) => {
                let mut last_allowed = None;
                for addr in tokio::net::lookup_host((x.as_str(), target.1)).await? {
                    let ip = addr.ip();
                    if Self::is_denied(ip, access) {
                        continue;
                    }
                    last_allowed = Some(addr);
                    break;
                }
                let Some(addr) = last_allowed else {
                    return Err("Target address blocked by ruleset!".into());
                };
                TcpStream::connect(addr).await?
            }
        };
        Ok(Self(stream))
    }

    fn ensure_allowed(ip: IpAddr, access: Option<&AccessState>) -> Result<()> {
        if Self::is_denied(ip, access) {
            return Err("Target address blocked by ruleset!".into());
        }
        Ok(())
    }

    fn is_denied(ip: IpAddr, access: Option<&AccessState>) -> bool {
        access.map(|x| x.is_denied(ip)).unwrap_or(false)
    }

    pub async fn connect_tcp(mut self, mut stream: TcpStream) -> Result<()> {
        tokio::io::copy_bidirectional(&mut self.0, &mut stream).await?;
        Ok(())
    }
}

impl Socks5Acceptor {
    pub async fn connect(mut self, target: Socks5Target) -> Result<()> {
        let client_addr = self.peer_addr();
        let client_addr_str = client_addr.to_string();
        let client_ip = client_addr.ip().to_string();
        let target_str = target.to_string();
        eprintln!("{client_addr} -> {target}");
        let connector = match Socks5TcpConnector::connect(&target, self.access.as_ref()).await {
            Ok(connector) => connector,
            Err(err) => {
                if err.to_string() == "Target address blocked by ruleset!" {
                    log_failure(
                        "access_denied",
                        self.username.as_deref(),
                        &client_addr_str,
                        &client_ip,
                        Some(&target_str),
                        "blocked_target_ip",
                    );
                    self.closed(2).await?;
                    return Err(err);
                }
                log_failure(
                    "connect_failed",
                    self.username.as_deref(),
                    &client_addr_str,
                    &client_ip,
                    Some(&target_str),
                    &err.to_string(),
                );
                return Err(err);
            }
        };
        let upstream_addr = connector.0.peer_addr()?;
        // RFC 1928: for CONNECT reply, BND.ADDR/BND.PORT should be the local
        // endpoint used to connect to the target server.
        self.connected(connector.0.local_addr()?).await?;
        log_access(
            "tcp",
            self.username.as_deref(),
            &client_addr_str,
            &client_ip,
            &upstream_addr.to_string(),
        );
        let result = connector.connect_tcp(self.stream).await;
        log_event(
            "connection_closed",
            self.username.as_deref(),
            &client_addr_str,
            &client_ip,
            Some(&upstream_addr.to_string()),
        );
        result
    }
}
