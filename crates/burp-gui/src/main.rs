#![allow(dead_code)]

use tracing_subscriber::layer::SubscriberExt;

fn main() {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(tracing_subscriber::filter::LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .finish()
        .with(tracing_tracy::TracyLayer::default());

    tracing::subscriber::set_global_default(subscriber).unwrap();

    tracing_log::LogTracer::init().unwrap();
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Burp",
        native_options,
        Box::new(|cc| Ok(Box::new(burp_gui::BurpApp::new("Burp", cc)))),
    )
    .unwrap();
}
