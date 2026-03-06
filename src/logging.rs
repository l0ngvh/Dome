use tracing_error::ErrorLayer;
use tracing_subscriber::fmt::format::DefaultFields;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt};

use crate::config::Config;

pub(crate) fn init_tracing(config: &Config) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("off,dome={}", config.log_level.as_str())));
    let log_dir = Config::log_dir();
    let file_layer = std::fs::create_dir_all(&log_dir)
        .and_then(|_| std::fs::File::create(format!("{log_dir}/dome.log")))
        .ok()
        .map(|f| {
            fmt::layer()
                .fmt_fields(FileFields(DefaultFields::new()))
                .with_ansi(false)
                .with_writer(std::sync::Mutex::new(f))
        });
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer())
        .with(file_layer)
        .with(ErrorLayer::default())
        .init();
}

// Newtype so the file layer gets its own FormattedFields span extension,
// avoiding ANSI bleed from the stdout layer.
struct FileFields(DefaultFields);

impl<'writer> fmt::FormatFields<'writer> for FileFields {
    fn format_fields<R: tracing_subscriber::field::RecordFields>(
        &self,
        writer: fmt::format::Writer<'writer>,
        fields: R,
    ) -> std::fmt::Result {
        self.0.format_fields(writer, fields)
    }
}
