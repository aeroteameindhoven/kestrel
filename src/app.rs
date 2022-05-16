use std::collections::BTreeMap;

use eframe::{
    egui::{
        self, CentralPanel, Context, Layout, RichText, SidePanel, TextFormat, TopBottomPanel,
        WidgetText,
    },
    emath::{Align2, Rect, Vec2},
    epaint::{text::LayoutJob, Color32, FontId, Shape, Stroke},
    App,
};
use egui_extras::{Size, TableBuilder};
use ringbuffer::{AllocRingBuffer, RingBuffer, RingBufferExt};

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
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
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

        SidePanel::left("metrics").min_width(250.0).show(ctx, |ui| {
            TopBottomPanel::top("reset").show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Reset Clock").clicked() {
                        self.current_time = 0;
                    }
                    if ui.button("Clear Latest Metrics").clicked() {
                        self.latest_metrics.clear();
                    }
                    if ui.button("Clear Packets").clicked() {
                        self.packets.clear();
                    }
                });
            });

            ui.heading(format!(
                "Current time: {}",
                metric_timestamp(self.current_time)
            ));
            ui.separator();

            ui.heading("Latest Metrics");
            TableBuilder::new(ui)
                .column(Size::remainder())
                .column(Size::initial(7.0 * 5.0))
                .column(Size::remainder())
                .striped(true)
                .cell_layout(Layout::left_to_right().with_main_wrap(false))
                .header(20.0, |mut header| {
                    header.col(|ui| {
                        ui.heading("Name");
                    });
                    header.col(|ui| {
                        ui.heading("Type");
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
                                ui.label(
                                    RichText::new(metric_value.ty())
                                        .monospace()
                                        .color(Color32::DARK_GREEN),
                                )
                                .on_hover_text_at_pointer(&last_update);
                            });
                            row.col(|ui| {
                                ui.monospace(metric_value.value())
                                    .on_hover_text_at_pointer(last_update);
                            });
                        });
                    }
                });

            ui.separator();
            ui.heading(format!("{} Packets", self.packets.len()));

            ui.push_id("Packets", |ui| {
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
                            let packet = &self.packets[-(idx as isize + 1)];

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
                                ui.monospace(
                                    timestamp.map_or_else(
                                        || "".into(),
                                        |&stamp| metric_timestamp(stamp),
                                    ),
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
        });

        CentralPanel::default()
            .frame(egui::Frame::dark_canvas(&ctx.style()))
            .show(ctx, |ui| {
                let canvas = ui.available_rect_before_wrap();

                let square_dimension = canvas.width().min(canvas.height());

                let robot_rect = Rect::from_center_size(
                    canvas.center(),
                    Vec2::new(square_dimension / 2.0, square_dimension * 3.0 / 5.0),
                );

                let robot = [
                    Shape::rect_filled(robot_rect, 0.0, Color32::WHITE.linear_multiply(0.5)),
                    Shape::rect_stroke(robot_rect, 0.0, Stroke::new(2.0, Color32::WHITE)),
                    Shape::text(
                        &ui.fonts(),
                        canvas.center(),
                        Align2::CENTER_CENTER,
                        "Robot",
                        FontId::monospace(26.0),
                        Color32::BLACK,
                    ),
                ];

                ui.painter().extend(robot.to_vec());

                if let Some((_, us_distance)) = self
                    .latest_metrics
                    .get(&MetricName::namespaced("ultrasonic", "distance"))
                {
                    let ultrasonic_vector = (square_dimension / 5.0)
                        * (us_distance.as_i128().unwrap_or_default() as f32 / 300.0)
                        * Vec2::Y;

                    let mut shapes = Shape::dashed_line(
                        &[
                            robot_rect.center_top(),
                            robot_rect.center_top() - ultrasonic_vector,
                        ],
                        Stroke::new(2.0, Color32::RED),
                        2.0,
                        2.0,
                    );

                    shapes.push(Shape::text(
                        &ui.fonts(),
                        robot_rect.center_top() - ultrasonic_vector,
                        Align2::CENTER_BOTTOM,
                        format!("{}cm", us_distance.value()),
                        FontId::monospace(15.0),
                        Color32::KHAKI,
                    ));

                    shapes.push(Shape::text(
                        &ui.fonts(),
                        robot_rect.center_top(),
                        Align2::CENTER_TOP,
                        "Ultrasonic Sensor",
                        FontId::monospace(15.0),
                        Color32::BLACK,
                    ));

                    ui.painter().extend(shapes);
                }

                if let Some((_, speed)) = self
                    .latest_metrics
                    .get(&MetricName::namespaced("motor", "drive_speed"))
                {
                    let speed = speed.as_f64().unwrap_or_default();

                    if speed.abs() > f64::EPSILON {
                        let (color, align, direction) = if speed > 0.0 {
                            (Color32::RED, Align2::CENTER_BOTTOM, Vec2::Y * -1.0)
                        } else {
                            (Color32::BLUE, Align2::CENTER_TOP, Vec2::Y)
                        };

                        let arrow_base = robot_rect.center() + 13.0 * direction;
                        let arrow_tip = arrow_base
                            + direction * ((robot_rect.height() / 4.0 * speed.abs() as f32) - 13.0);

                        let shapes = [
                            Shape::line(vec![arrow_base, arrow_tip], Stroke::new(7.0, color)),
                            Shape::text(
                                &ui.fonts(),
                                arrow_tip,
                                align,
                                format!("{speed:.3}"),
                                FontId::monospace(15.0),
                                color,
                            ),
                        ];

                        ui.painter().extend(shapes.to_vec());
                    }
                }
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
        MetricName::Global(name) => WidgetText::RichText(RichText::new(name).color(Color32::GOLD)),
    }
}
