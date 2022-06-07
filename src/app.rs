use std::collections::{BTreeMap, BTreeSet};

use eframe::{
    egui::{self, CentralPanel, Context, RichText, TopBottomPanel, Window},
    epaint::Color32,
    App,
};
use ringbuffer::{AllocRingBuffer, RingBuffer, RingBufferExt, RingBufferWrite};

use crate::{
    new_packet_ring_buffer,
    serial::{
        packet::{
            metric_name::MetricName, metric_value::MetricValue, timestamp::Timestamp, Packet,
        },
        worker::SerialWorkerController,
    },
    visualization::{
        focused_metrics::focused_metrics_plot, latest_metrics::latest_metrics,
        packets_table::packets_table, robot::robot,
    },
};

pub struct Application {
    pub pause_packets: bool,
    pub show_visualization: bool,
    pub connect_the_dots: bool,

    pub serial: SerialWorkerController,

    pub current_time: Timestamp,

    pub packets: AllocRingBuffer<Packet>,
    pub metrics_history: BTreeMap<MetricName, AllocRingBuffer<(Timestamp, MetricValue)>>,

    pub focused_metrics: BTreeSet<MetricName>,
}

impl App for Application {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        if !self.pause_packets {
            for new_packet in self.serial.new_packets() {
                if let Packet::Telemetry(metric) = &new_packet {
                    // Clear data if the arduino has rebooted
                    if metric.timestamp < self.current_time {
                        self.packets.clear();
                        self.metrics_history.clear();
                    }

                    // FIXME: TODO: tick clock when receiving no packets
                    self.current_time = metric.timestamp;

                    self.metrics_history
                        .entry(metric.name.clone())
                        .or_insert_with(new_packet_ring_buffer)
                        .push((metric.timestamp, metric.value.clone()));
                }

                self.packets.push(new_packet);
            }
        }

        TopBottomPanel::top("serial_info").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("Serial port {}", self.serial.port_name()));

                ui.separator();

                if self.serial.detached() {
                    if ui.button("Attach").clicked() {
                        self.serial.attach();
                    }

                    ui.label(RichText::new("Detached").color(Color32::RED));
                } else {
                    if ui.button("Detach").clicked() {
                        self.serial.detach();
                    }

                    ui.label(RichText::new("Attached").color(Color32::LIGHT_BLUE));

                    ui.separator();

                    if self.serial.connected() {
                        ui.label(RichText::new("Connected").color(Color32::GREEN));
                    } else {
                        ui.label(
                            RichText::new("Waiting for serial port to become available")
                                .color(Color32::YELLOW),
                        );

                        ui.spinner();
                    }
                }
            });
        });

        CentralPanel::default().show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("Reset Clock").clicked() {
                    self.current_time = Timestamp::default();
                }
                if ui.button("Clear Metrics History").clicked() {
                    self.metrics_history.clear();
                }
                if ui.button("Clear Packets").clicked() {
                    self.packets.clear();
                }
                if ui.button("Clear All").clicked() {
                    self.current_time = Timestamp::default();
                    self.metrics_history.clear();
                    self.packets.clear();
                }
                ui.checkbox(&mut self.show_visualization, "Show Visualization");

                ui.checkbox(&mut self.pause_packets, "Pause packets");
            });

            ui.heading(format!("Current time: {}", self.current_time));
            ui.separator();

            ui.heading("Latest Metrics");
            latest_metrics(
                ui,
                self.current_time,
                &mut self.focused_metrics,
                self.metrics_history.iter().filter_map(|(name, history)| {
                    history.back().map(|newest| (name, newest, history.len()))
                }),
            );

            ui.separator();
            if self.focused_metrics.is_empty() {
                ui.heading(format!("{} Packets", self.packets.len()));

                packets_table(ui, &self.packets)
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
                        self.metrics_history
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
                    self.metrics_history
                        .get(&metric_name)
                        .and_then(|metrics| metrics.back())
                        .map(|(_timestamp, value)| value)
                });
            });
    }
}
