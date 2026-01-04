use clap::{Parser, Subcommand};
use dome::{Action, Config, run_app};
use tracing_error::ErrorLayer;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt};

#[derive(Parser)]
#[command(name = "dome", about = "A cross-platform tiling window manager")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    Launch {
        #[arg(short, long)]
        config: Option<String>,
    },
    #[command(flatten)]
    Action(Action),
}

fn init_tracing(config: &Config) {
    let filter = config
        .log_level
        .as_ref()
        .and_then(|l| l.parse().ok())
        .unwrap_or_else(EnvFilter::from_default_env);
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer())
        .with(ErrorLayer::default())
        .init();
    std::panic::set_hook(Box::new(|panic_info| {
        let backtrace = backtrace::Backtrace::new();
        tracing::error!("Application panicked: {panic_info}. Backtrace: {backtrace:?}");
    }));
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        None => {
            let config = Config::load(None);
            init_tracing(&config);
            run_app(config);
        }
        Some(Command::Launch { config }) => {
            let cfg = Config::load(config.as_deref());
            init_tracing(&cfg);
            run_app(cfg);
        }
        Some(Command::Action(action)) => match dome::send_action(&action) {
            Ok(response) => println!("{response}"),
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        },
    }
}
