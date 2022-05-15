use std::collections::BTreeMap;

use eframe::{
    egui::{
        CentralPanel, Context, Layout, RichText, SidePanel, TextFormat, TopBottomPanel, WidgetText,
    },
    epaint::{text::LayoutJob, Color32},
    App, Frame,
};
use egui_extras::{Size, TableBuilder};
use ringbuffer::{AllocRingBuffer, RingBuffer};

use crate::serial::{
    packet::{Metric, MetricName, MetricValue, Packet},
    worker::SerialWorkerController,
};

pub struct Application {
    pub serial: SerialWorkerController,
    pub packets: AllocRingBuffer<Packet>,
    pub current_time: u32,
    pub latest_metrics: BTreeMap<MetricName, (u32, MetricValue)>,
}

impl App for Application {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        self.packets
            .extend(self.serial.new_packets().inspect(|packet| {
                if let Packet::Telemetry(metric) = packet {
                    self.latest_metrics.insert(
                        metric.name.clone(),
                        (metric.timestamp, metric.value.clone()),
                    );

                    self.current_time = metric.timestamp.max(self.current_time);
                }
            }));

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

        SidePanel::left("latest_metrics")
            .min_width(250.0)
            .show(ctx, |ui| {
                ui.heading(format!(
                    "Current time: {}",
                    metric_timestamp(self.current_time)
                ));
                if ui.button("Reset").clicked() {
                    self.current_time = 0;
                }
                ui.separator();

                ui.heading("Latest Metrics");
                if ui.button("Clear").clicked() {
                    self.latest_metrics.clear();
                }
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
                            let last_update = format!(
                                "Stale for {}",
                                metric_timestamp(self.current_time.saturating_sub(*timestamp))
                            );

                            body.row(15.0, |mut row| {
                                row.col(|ui| {
                                    ui.label(metric_name_text(metric_name))
                                        .on_hover_text_at_pointer(&last_update);
                                });
                                row.col(|ui| {
                                    // TODO: visualize stale data
                                    // let elapsed = OffsetDateTime::now_utc() - *timestamp;
                                    // let elapsed = elapsed.as_seconds_f32();

                                    // let color =
                                    //     Color32::GREEN.linear_multiply((1.0 - elapsed).max(0.0));

                                    ui.monospace(format!("{metric_value:?}"))
                                        .on_hover_text_at_pointer(last_update);
                                });
                            });
                        }
                    });
            });

        TopBottomPanel::bottom("telemetry_log")
            .resizable(true)
            .show(ctx, |ui| {
                TableBuilder::new(ui)
                    .column(Size::exact(7.0 * 9.0))
                    .columns(Size::remainder(), 2)
                    .striped(true)
                    .cell_layout(Layout::left_to_right().with_main_wrap(false))
                    .header(20.0, |mut header| {
                        header.col(|ui| {
                            ui.heading("Time").on_hover_text_at_pointer(
                                "Time since the robot has been powered up",
                            );
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
                            let packet = &self.packets[- (idx as isize + 1)];

                            let (timestamp, name, data) = match packet {
                                Packet::Telemetry(Metric {
                                    timestamp,
                                    value,
                                    name,
                                }) => (
                                    Some(timestamp),
                                    metric_name_text(name),
                                    RichText::new(format!("{value:?}")).monospace(),
                                ),
                                Packet::System(packet) => (
                                    None,
                                    // TODO: non row element?
                                    RichText::new("[system]").color(Color32::YELLOW).into(),
                                    RichText::new(format!("{packet:?}"))
                                        .monospace()
                                        .color(Color32::KHAKI),
                                ),
                            };

                            row.col(|ui| {
                                ui.monospace(timestamp.map_or_else(
                                    || "N/A".into(),
                                    |&stamp| metric_timestamp(stamp),
                                ));
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

fn metric_timestamp(timestamp: u32) -> String {
    let millis = timestamp % 1_000;
    let seconds = (timestamp / 1_000) % 60;
    let minutes = timestamp / 60_000;

    format!("{minutes:02}:{seconds:02}.{millis:03}")
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
