use dome::run_app;
use tracing_error::ErrorLayer;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, layer::SubscriberExt};

fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(ErrorLayer::default())
        .init();
    std::panic::set_hook(Box::new(|panic_info| {
        let backtrace = backtrace::Backtrace::new();
        tracing::error!("Application panicked: {panic_info}. Backtrace: {backtrace:?}");
    }));

    run_app();
}
