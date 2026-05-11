use std::collections::HashSet;
use std::convert::TryFrom;
use std::fmt::{self, Display, Formatter};
use std::io::{ErrorKind, IoSlice};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::pin::Pin;
use std::task::{Context, Poll};

use clap::Parser;
use indoc::indoc;
use socket2::{Domain, Protocol, Socket, Type};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs, UdpSocket};
use tokio_stream::{Stream, StreamExt};

#[cfg(target_family = "unix")]
use self::util::set_rlimit_nofile;
use self::{
    acceptor::Socks5Acceptor,
    config::AppConfig,
    error::{Error, Result},
    logger::{init as init_logger, log_event, log_failure},
    listener::Socks5Listener,
    target::{Socks5Host, Socks5Target},
    util::{IntoResult, PutSocks5Addr, Split},
};

pub type Socks5Stream = TcpStream;

mod acceptor;
mod config;
mod error;
mod listener;
mod logger;
mod target;
mod tcp;
mod udp;
mod util;

#[derive(Parser, Debug)]
#[command(
    version,
    author,
    about,
    help_template = indoc! {"
        {before-help}{name} {version}
        {author}
        {about}

        {usage-heading} {usage}

        {all-args}{after-help}
    "}
)]
struct Cli {
    #[arg(
        short = 'c',
        long = "config",
        value_name = "FILE",
        help = "Path to TOML configuration file",
        required = true
    )]
    config: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = AppConfig::from_file(&cli.config)?;
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

    while let Some((mut acceptor, client)) = listener.next().await.transpose()? {
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

    Ok(())
}
