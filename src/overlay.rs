use crate::data_source::{BleCommand, BleDevice, DataEvent};
use crate::log_watcher::LogEvent;
use crate::metrics::PowerMetrics;
use eframe::egui;
use std::sync::mpsc;

enum Screen {
    DevicePicker,
    Connecting,
    Overlay,
}

pub struct OverlayApp {
    metrics: PowerMetrics,
    ble_rx: mpsc::Receiver<DataEvent>,
    ble_cmd_tx: mpsc::Sender<BleCommand>,
    log_rx: mpsc::Receiver<LogEvent>,
    screen: Screen,
    devices: Vec<BleDevice>,
    connected_name: String,
    connecting_name: String,
    error_msg: Option<String>,
    current_power: f64,
    current_interval: u32,
}

impl OverlayApp {
    pub fn new(
        ble_rx: mpsc::Receiver<DataEvent>,
        ble_cmd_tx: mpsc::Sender<BleCommand>,
        log_rx: mpsc::Receiver<LogEvent>,
    ) -> Self {
        Self {
            metrics: PowerMetrics::new(),
            ble_rx,
            ble_cmd_tx,
            log_rx,
            screen: Screen::DevicePicker,
            devices: Vec::new(),
            connected_name: String::new(),
            connecting_name: String::new(),
            error_msg: None,
            current_power: 0.0,
            current_interval: 0,
        }
    }

    fn drain_events(&mut self) {
        while let Ok(event) = self.ble_rx.try_recv() {
            match event {
                DataEvent::Power(watts) => {
                    self.current_power = watts;
                    self.metrics.record(watts);
                }
                DataEvent::DeviceList(devices) => {
                    self.devices = devices;
                }
                DataEvent::Connected(name) => {
                    self.connected_name = name;
                    self.error_msg = None;
                    self.screen = Screen::Overlay;
                }
                DataEvent::Disconnected => {
                    self.screen = Screen::DevicePicker;
                }
                DataEvent::Error(msg) => {
                    self.error_msg = Some(msg);
                    self.screen = Screen::DevicePicker;
                }
            }
        }

        while let Ok(event) = self.log_rx.try_recv() {
            match event {
                LogEvent::IntervalStarted(n) => {
                    self.current_interval = n;
                    self.metrics.start_interval();
                }
            }
        }
    }
}

impl eframe::App for OverlayApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_events();
        ctx.request_repaint_after(std::time::Duration::from_millis(250));

        let bg = egui::Color32::from_rgba_premultiplied(20, 20, 30, 230);

        match self.screen {
            Screen::DevicePicker => self.show_device_picker(ctx, bg),
            Screen::Connecting => self.show_connecting(ctx, bg),
            Screen::Overlay => self.show_overlay(ctx, bg),
        }
    }
}

impl OverlayApp {
    fn show_device_picker(&mut self, ctx: &egui::Context, bg: egui::Color32) {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(bg)
                    .inner_margin(12.0)
                    .corner_radius(8.0),
            )
            .show(ctx, |ui| {
                ui.visuals_mut().override_text_color = Some(egui::Color32::WHITE);

                ui.label(
                    egui::RichText::new("Select Power Meter")
                        .color(egui::Color32::from_rgb(255, 165, 0))
                        .size(16.0)
                        .strong(),
                );

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(4.0);

                if let Some(err) = &self.error_msg {
                    ui.label(
                        egui::RichText::new(err.as_str())
                            .color(egui::Color32::from_rgb(255, 80, 80))
                            .size(11.0),
                    );
                    ui.add_space(4.0);
                }

                if self.devices.is_empty() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Scanning for exercise devices...");
                    });
                } else {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(
                            egui::RichText::new(format!("{} device(s) found", self.devices.len()))
                                .color(egui::Color32::from_rgb(140, 140, 140))
                                .size(11.0),
                        );
                    });
                    ui.add_space(4.0);

                    let mut connect_id = None;

                    egui::ScrollArea::vertical()
                        .max_height(250.0)
                        .show(ui, |ui| {
                            for device in &self.devices {
                                let btn = ui.add_sized(
                                    [ui.available_width(), 28.0],
                                    egui::Button::new(
                                        egui::RichText::new(&device.name)
                                            .size(13.0)
                                            .color(egui::Color32::from_rgb(20, 20, 30)),
                                    ),
                                );
                                if btn.clicked() {
                                    connect_id = Some(device.id.clone());
                                }
                            }
                        });

                    if let Some(id) = connect_id {
                        // Find the device name for the connecting screen
                        if let Some(dev) = self.devices.iter().find(|d| d.id == id) {
                            self.connecting_name = dev.name.clone();
                        }
                        let _ = self.ble_cmd_tx.send(BleCommand::Connect(id));
                        self.screen = Screen::Connecting;
                    }
                }
            });
    }

    fn show_connecting(&self, ctx: &egui::Context, bg: egui::Color32) {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(bg)
                    .inner_margin(12.0)
                    .corner_radius(8.0),
            )
            .show(ctx, |ui| {
                ui.visuals_mut().override_text_color = Some(egui::Color32::WHITE);
                ui.add_space(20.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spinner();
                    ui.label(
                        egui::RichText::new(format!("Connecting to {}...", self.connecting_name))
                            .size(12.0),
                    );
                });
            });
    }

    fn show_overlay(&mut self, ctx: &egui::Context, bg: egui::Color32) {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(bg)
                    .inner_margin(12.0)
                    .corner_radius(8.0),
            )
            .show(ctx, |ui| {
                ui.visuals_mut().override_text_color = Some(egui::Color32::WHITE);

                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Zwift Power")
                            .color(egui::Color32::from_rgb(255, 165, 0))
                            .size(16.0)
                            .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let btn = ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(&self.connected_name)
                                        .color(egui::Color32::from_rgb(0, 200, 0))
                                        .size(11.0),
                                )
                                .frame(false),
                            )
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        if btn.clicked() {
                            let _ = self.ble_cmd_tx.send(BleCommand::Disconnect);
                        }
                    });
                });

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{:.0}", self.current_power))
                            .color(power_color(self.current_power))
                            .size(36.0)
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new("W")
                            .color(egui::Color32::from_rgb(140, 140, 140))
                            .size(16.0),
                    );
                });

                ui.add_space(6.0);

                let session_avg = self.metrics.session_avg_power();
                let np = self.metrics.normalized_power();
                metric_row(ui, "Session Avg", session_avg);
                metric_row(ui, "Session NP", np);

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                let interval_avg = self.metrics.interval_avg_power();
                let label = if self.current_interval > 0 {
                    format!("Interval {}", self.current_interval)
                } else {
                    "Interval".to_string()
                };
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(label)
                            .color(egui::Color32::from_rgb(140, 140, 140))
                            .size(13.0),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let color = if self.metrics.in_interval {
                            power_color(interval_avg)
                        } else {
                            egui::Color32::from_rgb(80, 80, 80)
                        };
                        ui.label(
                            egui::RichText::new(format!("{interval_avg:.0} W"))
                                .color(color)
                                .size(13.0)
                                .strong(),
                        );
                    });
                });
            });
    }
}

fn metric_row(ui: &mut egui::Ui, label: &str, watts: f64) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(label)
                .color(egui::Color32::from_rgb(140, 140, 140))
                .size(13.0),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(format!("{watts:.0} W"))
                    .color(power_color(watts))
                    .size(13.0)
                    .strong(),
            );
        });
    });
}

fn power_color(watts: f64) -> egui::Color32 {
    if watts < 1.0 {
        egui::Color32::from_rgb(80, 80, 80)
    } else if watts < 100.0 {
        egui::Color32::from_rgb(150, 200, 255)
    } else if watts < 200.0 {
        egui::Color32::from_rgb(200, 255, 200)
    } else if watts < 300.0 {
        egui::Color32::from_rgb(255, 255, 100)
    } else if watts < 400.0 {
        egui::Color32::from_rgb(255, 165, 0)
    } else {
        egui::Color32::from_rgb(255, 60, 60)
    }
}
