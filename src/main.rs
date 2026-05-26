use clap::Parser;
use indoc::indoc;
use tokio::sync::watch;

use sock5s::{error::Result, run_server_from_path};

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
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    run_server_from_path(&cli.config, shutdown_rx).await
}
