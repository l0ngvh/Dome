use clap::{Parser, Subcommand};
use dome::{Action, DomeClient, Query, run_app};

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
    Query {
        #[command(subcommand)]
        query: Query,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        None => run_app(None)?,
        Some(Command::Launch { config }) => run_app(config)?,
        Some(Command::Action(action)) => {
            DomeClient.send_action(&action)?;
        }
        Some(Command::Query { query }) => {
            let response = DomeClient.send_query(&query)?;
            println!("{response}");
        }
    }
    Ok(())
}
