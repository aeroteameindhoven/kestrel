use std::collections::{BTreeMap, BTreeSet};

use eframe::{
    egui::{self, CentralPanel, Context, RichText, TopBottomPanel, Window},
    epaint::Color32,
    App,
};
use ringbuffer::{AllocRingBuffer, RingBuffer, RingBufferExt, RingBufferWrite};

use crate::{
    new_metric_ring_buffer,
    serial::{
        metric::{name::MetricName, timestamp::Timestamp, value::MetricValue, Metric},
        worker::{SerialWorkerController, SerialWorkerState},
    },
    visualization::{
        focused_metrics::focused_metrics_plot, latest_metrics::latest_metrics,
        metrics_history::metrics_history, robot::robot,
    },
};

pub struct Application {
    pub pause_metrics: bool,
    pub show_visualization: bool,
    pub connect_the_dots: bool,

    pub serial: SerialWorkerController,

    pub current_time: Timestamp,

    pub raw_metrics: AllocRingBuffer<Metric>,
    pub sorted_metrics: BTreeMap<MetricName, AllocRingBuffer<(Timestamp, MetricValue)>>,

    pub focused_metrics: BTreeSet<MetricName>,
}

impl App for Application {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        if !self.pause_metrics {
            for metric in self.serial.new_metrics() {
                // Clear data if the arduino has rebooted
                if metric.timestamp < self.current_time {
                    self.raw_metrics.clear();
                    self.sorted_metrics.clear();
                }

                // FIXME: TODO: tick clock when receiving no metrics
                self.current_time = metric.timestamp;

                self.sorted_metrics
                    .entry(metric.name.clone())
                    .or_insert_with(new_metric_ring_buffer)
                    .push((metric.timestamp, metric.value.clone()));

                self.raw_metrics.push(metric);
            }
        }

        TopBottomPanel::top("serial_info").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("Serial port {}", self.serial.port_name()));

                ui.separator();

                match self.serial.state() {
                    SerialWorkerState::Detached => {
                        if ui.button("Watch Serial").clicked() {
                            self.serial.attach();
                        }

                        ui.label(RichText::new("Ignoring Serial").color(Color32::RED));
                    }
                    SerialWorkerState::Connected => {
                        if ui.button("Disconnect").clicked() {
                            self.serial.detach();
                        }

                        ui.label(RichText::new("Connected").color(Color32::GREEN));

                        ui.separator();

                        ui.add_enabled_ui(
                            self.serial.state() == SerialWorkerState::Connected,
                            |ui| {
                                if ui.button("Reset Arduino").clicked() {
                                    self.serial.reset();
                                }
                            },
                        );
                    }
                    SerialWorkerState::Disconnected => {
                        if ui.button("Stop Waiting").clicked() {
                            self.serial.detach();
                        }

                        ui.label(
                            RichText::new("Waiting for serial port to become available")
                                .color(Color32::YELLOW),
                        );

                        ui.spinner();
                    }
                    SerialWorkerState::Resetting => {
                        ui.label(RichText::new("Resetting").color(Color32::LIGHT_BLUE));

                        ui.spinner();
                    }
                }
            });
        });

        CentralPanel::default().show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading(format!("Current time: {}", self.current_time));

                if ui.button("Clear Memory").clicked() {
                    self.current_time = Timestamp::default();
                    self.sorted_metrics.clear();
                    self.raw_metrics.clear();
                }

                ui.toggle_value(&mut self.show_visualization, "Show Visualization");
                ui.toggle_value(&mut self.pause_metrics, "Pause metric ingest");
            });

            ui.separator();

            ui.heading(format!("{} Latest Metrics", self.sorted_metrics.len()));
            latest_metrics(
                ui,
                self.current_time,
                &mut self.focused_metrics,
                self.sorted_metrics.iter().filter_map(|(name, history)| {
                    history.back().map(|newest| (name, newest, history.len()))
                }),
            );

            ui.separator();
            if self.focused_metrics.is_empty() {
                ui.heading(format!("{} Historical Metrics", self.raw_metrics.len()));

                metrics_history(ui, &self.raw_metrics)
            } else {
                ui.horizontal_wrapped(|ui| {
                    ui.checkbox(&mut self.connect_the_dots, "Connect The Dots?");
                    if ui.button("Clear Focus").clicked() {
                        self.focused_metrics.clear();
                    }
                    ui.label("Focused: ");
                    for focused in &self.focused_metrics {
                        ui.label(focused);
                    }
                });
                ui.collapsing("Plot Instructions", |ui| {
                    ui.label("Pan by dragging, or scroll (+ shift = horizontal).");
                    ui.label("Box zooming: Right click to zoom in and zoom out using a selection.");
                    if cfg!(target_arch = "wasm32") {
                        ui.label("Zoom with ctrl / ⌘ + pointer wheel, or with pinch gesture.");
                    } else if cfg!(target_os = "macos") {
                        ui.label("Zoom with ctrl / ⌘ + scroll.");
                    } else {
                        ui.label("Zoom with ctrl + scroll.");
                    }
                    ui.label("Reset view with double-click.");
                });

                focused_metrics_plot(
                    ui,
                    self.focused_metrics.iter().filter_map(|metric_name| {
                        self.sorted_metrics
                            .get(metric_name)
                            .map(|metric_values| (metric_name, metric_values.iter()))
                    }),
                    self.connect_the_dots,
                );
            }
        });

        Window::new("Visualization")
            .open(&mut self.show_visualization)
            .frame(egui::Frame::dark_canvas(&ctx.style()))
            .show(ctx, |ui| {
                robot(ui, |metric_name| {
                    self.sorted_metrics
                        .get(&metric_name)
                        .and_then(|metrics| metrics.back())
                        .map(|(_timestamp, value)| value)
                });
            });
    }
}
