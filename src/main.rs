use clap::{Parser, Subcommand};
use dome::{Action, run_app};

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
        None => run_app(None),
        Some(Command::Launch { config }) => run_app(config),
        Some(Command::Action(action)) => match dome::send_action(&action) {
            Ok(response) => println!("{response}"),
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        },
    }
}
