mod data_source;
mod log_watcher;
mod metrics;
mod overlay;

use data_source::run_ble;
use log_watcher::{default_log_path, watch_zwift_log};
use overlay::OverlayApp;
use std::sync::mpsc;

fn main() -> eframe::Result<()> {
    // BLE data -> GUI
    let (ble_tx, ble_rx) = mpsc::channel();

    // GUI commands -> BLE
    let (cmd_tx, cmd_rx) = mpsc::channel();

    // Log watcher -> GUI
    let (log_tx, log_rx) = mpsc::channel();

    // Spawn BLE on async runtime
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime");
    rt.spawn(async move {
        run_ble(ble_tx, cmd_rx).await;
    });

    // Spawn log watcher
    let log_path = default_log_path();
    std::thread::spawn(move || {
        watch_zwift_log(log_tx, log_path);
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([260.0, 300.0])
            .with_min_inner_size([220.0, 200.0])
            .with_always_on_top()
            .with_transparent(true)
            .with_decorations(true)
            .with_icon(app_icon()),
        ..Default::default()
    };

    eframe::run_native(
        "Zwift Power Overlay",
        options,
        Box::new(move |_cc| Ok(Box::new(OverlayApp::new(ble_rx, cmd_tx, log_rx)))),
    )
}

fn app_icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../assets/app.iconset/icon_128x128@2x.png"))
        .expect("app icon PNG must be valid")
}
