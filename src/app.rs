use std::collections::HashMap;

use eframe::{
    egui::{CentralPanel, Context, RichText, SidePanel, TopBottomPanel},
    epaint::Color32,
    App, Frame,
};
use egui_extras::{Size, TableBuilder};
use time::OffsetDateTime;

use crate::serial::{
    packet::{Metric, MetricValue, Packet},
    worker::SerialWorkerController,
};

pub struct Application {
    pub serial: SerialWorkerController,
    pub packets: Vec<(OffsetDateTime, Packet)>,
    pub latest_metrics: HashMap<String, MetricValue>,
}

impl App for Application {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        self.packets
            .extend(self.serial.new_packets().inspect(|(_, packet)| {
                if let Packet::Telemetry(metric) = packet {
                    self.latest_metrics
                        .insert(metric.name.clone(), metric.value.clone());
                }
            }));

        TopBottomPanel::top("serial_select").show(ctx, |ui| {
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
                    
                    ui.label(RichText::new("Attached").color(Color32::YELLOW));

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

        SidePanel::left("latest_metrics").show(ctx, |ui| {
            ui.heading("Latest Metrics");
            ui.separator();

            TableBuilder::new(ui)
                .columns(Size::remainder(), 2)
                .striped(true)
                .header(20.0, |mut header| {
                    header.col(|ui| {
                        ui.heading("Name");
                    });
                    header.col(|ui| {
                        ui.heading("Value");
                    });
                })
                .body(|mut body| {
                    for (metric_name, metric_value) in self.latest_metrics.iter() {
                        body.row(15.0, |mut row| {
                            row.col(|ui| {
                                ui.label(metric_name);
                            });
                            row.col(|ui| {
                                ui.monospace(format!("{metric_value:?}"));
                            });
                        });
                    }
                });
        });

        CentralPanel::default().show(ctx, |ui| {
            TableBuilder::new(ui)
                .columns(Size::remainder(), 3)
                .striped(true)
                .header(20.0, |mut header| {
                    header.col(|ui| {
                        ui.heading("Timestamp");
                    });
                    header.col(|ui| {
                        ui.heading("Metric Name");
                    });
                    header.col(|ui| {
                        ui.heading("Metric Data");
                    });
                })
                .body(|body| {
                    body.rows(15.0, self.packets.len(), |idx, mut row| {
                        let (timestamp, packet) = &self.packets[self.packets.len() - (idx + 1)];

                        let (name, data) = match packet {
                            Packet::Telemetry(Metric {
                                value: metric,
                                name: metric_name,
                            }) => (
                                RichText::new(metric_name),
                                RichText::new(format!("{metric:?}")).monospace(),
                            ),
                            Packet::System(packet) => (
                                // TODO: non row element?
                                RichText::new("[system]").color(Color32::YELLOW),
                                RichText::new(format!("{packet:?}"))
                                    .monospace()
                                    .color(Color32::KHAKI),
                            ),
                        };

                        row.col(|ui| {
                            ui.label(timestamp.to_string());
                        });
                        row.col(|ui| {
                            ui.label(name);
                        });
                        row.col(|ui| {
                            ui.label(data);
                        });
                    })
                });
        });
    }
}
