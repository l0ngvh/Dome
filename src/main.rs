use anyhow::Result;
use new_wm::WindowManager;
use tracing_error::ErrorLayer;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, layer::SubscriberExt};

fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer().with_span_events(FmtSpan::ENTER))
        .with(ErrorLayer::default())
        .init();
    std::panic::set_hook(Box::new(|panic_info| {
        let backtrace = backtrace::Backtrace::new();
        tracing::error!("Application panicked: {panic_info}. Backtrace: {backtrace:?}");
    }));
    if let Err(e) = run() {
        eprintln!("{e:#}");
    }
}

fn run() -> Result<()> {
    let wm = WindowManager::new()?;
    wm.list_windows()
}
