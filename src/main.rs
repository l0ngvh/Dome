use clap::{Parser, Subcommand};
use dome::{Action, run_app};
use tracing_error::ErrorLayer;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, layer::SubscriberExt};

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

fn main() {
    let cli = Cli::parse();

    match cli.command {
        None => {
            tracing_subscriber::registry()
                .with(fmt::layer())
                .with(ErrorLayer::default())
                .init();
            std::panic::set_hook(Box::new(|panic_info| {
                let backtrace = backtrace::Backtrace::new();
                tracing::error!("Application panicked: {panic_info}. Backtrace: {backtrace:?}");
            }));

            run_app(None);
        }
        Some(Command::Launch { config }) => {
            tracing_subscriber::registry()
                .with(fmt::layer())
                .with(ErrorLayer::default())
                .init();
            std::panic::set_hook(Box::new(|panic_info| {
                let backtrace = backtrace::Backtrace::new();
                tracing::error!("Application panicked: {panic_info}. Backtrace: {backtrace:?}");
            }));

            run_app(config);
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
