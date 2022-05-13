use std::collections::BTreeMap;

use eframe::{
    egui::{
        CentralPanel, Context, Layout, RichText, SidePanel, TextFormat, TopBottomPanel, WidgetText,
    },
    epaint::{text::LayoutJob, Color32},
    App, Frame,
};
use egui_extras::{Size, TableBuilder};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::serial::{
    packet::{Metric, MetricName, MetricValue, Packet},
    worker::SerialWorkerController,
};

pub struct Application {
    pub serial: SerialWorkerController,
    pub packets: Vec<(OffsetDateTime, Packet)>,
    pub latest_metrics: BTreeMap<MetricName, (OffsetDateTime, MetricValue)>,
}

impl App for Application {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        self.packets
            .extend(self.serial.new_packets().inspect(|(timestamp, packet)| {
                if let Packet::Telemetry(metric) = packet {
                    self.latest_metrics
                        .insert(metric.name.clone(), (*timestamp, metric.value.clone()));
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

        SidePanel::left("latest_metrics")
            .min_width(250.0)
            .show(ctx, |ui| {
                ui.heading("Latest Metrics");
                ui.separator();

                TableBuilder::new(ui)
                    .columns(Size::remainder(), 2)
                    .striped(true)
                    .cell_layout(Layout::left_to_right().with_main_wrap(false))
                    .header(20.0, |mut header| {
                        header.col(|ui| {
                            ui.heading("Name");
                        });
                        header.col(|ui| {
                            ui.heading("Value");
                        });
                    })
                    .body(|mut body| {
                        for (metric_name, (timestamp, metric_value)) in self.latest_metrics.iter() {
                            body.row(15.0, |mut row| {
                                row.col(|ui| {
                                    ui.label(metric_name_text(metric_name));
                                });
                                row.col(|ui| {
                                    // TODO: visualize stale data
                                    // let elapsed = OffsetDateTime::now_utc() - *timestamp;
                                    // let elapsed = elapsed.as_seconds_f32();

                                    // let color =
                                    //     Color32::GREEN.linear_multiply((1.0 - elapsed).max(0.0));

                                    ui.monospace(format!("{metric_value:?}"));
                                });
                            });
                        }
                    });
            });

        CentralPanel::default().show(ctx, |ui| {
            TableBuilder::new(ui)
                .column(Size::exact(7.0 * 30.0))
                .columns(Size::remainder(), 2)
                .striped(true)
                .cell_layout(Layout::left_to_right().with_main_wrap(false))
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
                                metric_name_text(metric_name),
                                RichText::new(format!("{metric:?}")).monospace(),
                            ),
                            Packet::System(packet) => (
                                // TODO: non row element?
                                RichText::new("[system]").color(Color32::YELLOW).into(),
                                RichText::new(format!("{packet:?}"))
                                    .monospace()
                                    .color(Color32::KHAKI),
                            ),
                        };

                        row.col(|ui| {
                            ui.monospace(
                                timestamp
                                    .format(&Rfc3339)
                                    .expect("RFC3339 should never fail to format"),
                            );
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

fn metric_name_text(name: &MetricName) -> WidgetText {
    match name {
        MetricName::Namespaced { namespace, name } => {
            let mut job = LayoutJob::default();
            job.append(
                namespace,
                0.0,
                TextFormat {
                    color: Color32::KHAKI,
                    ..Default::default()
                },
            );
            job.append(":", 0.0, Default::default());
            job.append(
                name,
                0.0,
                TextFormat {
                    color: Color32::GOLD,
                    ..Default::default()
                },
            );
            WidgetText::LayoutJob(job)
        }
        MetricName::Default(name) => WidgetText::RichText(RichText::new(name).color(Color32::GOLD)),
    }
}
