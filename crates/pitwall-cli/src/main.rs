//! `pitwall` 실행 파일. 인자가 없으면 서버를 띄운다.

mod embed;
mod server;

use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "pitwall", version = pitwall_core::VERSION, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// 보드 서버를 실행한다 (기본 명령)
    Serve {
        /// 사용할 포트
        #[arg(long, default_value_t = server::DEFAULT_PORT)]
        port: u16,
    },
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let cli = Cli::parse();
    match cli.command.unwrap_or(Command::Serve {
        port: server::DEFAULT_PORT,
    }) {
        Command::Serve { port } => server::serve(port).await,
    }
}
