use std::collections::HashSet;
use std::convert::TryFrom;
use std::fmt::{self, Display, Formatter};
use std::io::{ErrorKind, IoSlice};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::pin::Pin;
use std::task::{Context, Poll};

use socket2::{Domain, Protocol, Socket, Type};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs, UdpSocket};
use tokio::sync::watch;
use tokio_stream::{Stream, StreamExt};

#[cfg(target_family = "unix")]
use self::util::set_rlimit_nofile;
use self::{
    acceptor::Socks5Acceptor,
    config::AppConfig,
    error::{Error, Result},
    listener::Socks5Listener,
    logger::{init as init_logger, log_event, log_failure},
    target::{Socks5Host, Socks5Target},
    util::{IntoResult, PutSocks5Addr, Split},
};

pub type Socks5Stream = TcpStream;

pub mod acceptor;
pub mod config;
pub mod error;
pub mod listener;
pub mod logger;
pub mod target;
pub mod tcp;
pub mod udp;
pub mod util;

pub async fn run_server_from_path(config_path: &str, shutdown: watch::Receiver<bool>) -> Result<()> {
    let config = AppConfig::from_file(config_path)?;
    run_server(config, shutdown).await
}

pub async fn run_server(config: AppConfig, mut shutdown: watch::Receiver<bool>) -> Result<()> {
    init_logger(config.log.clone())?;
    let auth = config.auth.to_state();
    let access = config.access.to_state();
    let mut listener = Socks5Listener::listen(config.listen).await?;
    log_event(
        "server_started",
        None,
        &config.listen.to_string(),
        &config.listen.ip().to_string(),
        None,
    );

    #[cfg(target_family = "unix")]
    let _ = set_rlimit_nofile(4096);

    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                match changed {
                    Ok(_) if *shutdown.borrow() => break,
                    Ok(_) => continue,
                    Err(_) => break,
                }
            }
            maybe_client = listener.next() => {
                let Some((mut acceptor, client)) = maybe_client.transpose()? else {
                    break;
                };
                acceptor.auth = auth.clone();
                acceptor.access = access.clone();
                tokio::spawn(async move {
                    if let Err(e) = acceptor.accept().await {
                        log_failure(
                            "session_error",
                            None,
                            &client.to_string(),
                            &client.ip().to_string(),
                            None,
                            &e.to_string(),
                        );
                    }
                });
            }
        }
    }

    log_event(
        "server_stopped",
        None,
        &config.listen.to_string(),
        &config.listen.ip().to_string(),
        None,
    );

    Ok(())
}
