fn main() -> Result<(), eframe::Error> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "hdaw=info".into()),
        )
        .init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_title("HDAW"),
        ..Default::default()
    };

    eframe::run_native(
        "HDAW",
        options,
        Box::new(|_cc| Ok(Box::new(hdaw::app::HdawApp::new()))),
    )
}
