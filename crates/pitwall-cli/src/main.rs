//! `pitwall` 실행 파일. 인자가 없으면 서버를 띄운다.

mod embed;
mod hook;
mod init;
mod runtime;
mod server;

use std::path::PathBuf;
use std::process::ExitCode;

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
        /// 사용할 포트 (기본: config.toml의 port, 없으면 7311)
        #[arg(long)]
        port: Option<u16>,
        /// 브라우저를 자동으로 열지 않는다
        #[arg(long)]
        no_open: bool,
    },
    /// Claude Code 사용자 설정에 pitwall hook을 설치한다
    Init {
        /// diff 확인 없이 바로 적용
        #[arg(long)]
        yes: bool,
        /// pitwall이 추가한 hook만 제거
        #[arg(long)]
        uninstall: bool,
        /// 대상 settings.json (기본: ~/.claude/settings.json)
        #[arg(long, value_name = "PATH")]
        settings: Option<PathBuf>,
    },
    /// Claude Code hook에서 호출된다. 직접 실행할 일은 없다
    #[command(hide = true)]
    Hook {
        /// hook 이벤트 이름 (예: Stop)
        event: String,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command.unwrap_or(Command::Serve {
        port: None,
        no_open: false,
    }) {
        // hook은 stdout을 Claude Code가 읽으므로 로그를 켜지 않고 항상 exit 0.
        Command::Hook { event } => {
            hook::run(&event);
            ExitCode::SUCCESS
        }
        Command::Init {
            yes,
            uninstall,
            settings,
        } => run_init(yes, uninstall, settings),
        Command::Serve { port, no_open } => run_serve(port, !no_open),
    }
}

fn run_serve(port: Option<u16>, open_browser: bool) -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("pitwall: failed to start runtime: {e}");
            return ExitCode::FAILURE;
        }
    };
    let opts = server::ServeOptions {
        config_path: pitwall_core::project::Config::default_path(),
        port,
        open_browser,
    };
    match runtime.block_on(server::serve(opts)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("pitwall: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run_init(yes: bool, uninstall: bool, settings: Option<PathBuf>) -> ExitCode {
    let Some(settings) = settings.or_else(pitwall_core::paths::claude_user_settings) else {
        eprintln!("pitwall: could not find your home directory. Use --settings <PATH>.");
        return ExitCode::FAILURE;
    };
    let exe = match std::env::current_exe() {
        Ok(p) => pitwall_core::paths::normalize(&p),
        Err(e) => {
            eprintln!("pitwall: could not find the pitwall executable path: {e}");
            return ExitCode::FAILURE;
        }
    };
    let opts = init::Options {
        settings,
        exe,
        yes,
        uninstall,
    };
    let stdin = std::io::stdin();
    match init::run(&opts, &mut std::io::stdout(), &mut stdin.lock()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("pitwall: {e}");
            ExitCode::FAILURE
        }
    }
}
