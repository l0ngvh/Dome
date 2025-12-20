use anyhow::Result;
use new_wm::WindowManager;
use tracing_error::ErrorLayer;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, layer::SubscriberExt};

fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(ErrorLayer::default())
        .init();
    println!("Window Manager starting...");
    if let Err(e) = run() {
        eprintln!("{e:#}");
    }
}

#[tracing::instrument]
fn run() -> Result<()> {
    let wm = WindowManager::new()?;
    wm.list_windows()
}
