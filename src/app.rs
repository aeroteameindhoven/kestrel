use std::{
    collections::{hash_map::DefaultHasher, BTreeMap, BTreeSet},
    hash::{Hash, Hasher},
};

use eframe::{
    egui::{
        self,
        plot::{uniform_grid_spacer, Legend, Line, Plot, Points, Value, Values},
        CentralPanel, Context, Layout, RichText, Sense, TextFormat, TopBottomPanel, WidgetText,
        Window,
    },
    emath::{Align2, Rect, Vec2},
    epaint::{color::Hsva, text::LayoutJob, Color32, FontId, Shape, Stroke},
    App,
};
use egui_extras::{Size, TableBuilder};
use ringbuffer::{AllocRingBuffer, RingBuffer, RingBufferExt, RingBufferWrite};

use crate::{
    new_packet_ring_buffer,
    serial::{
        packet::{Metric, MetricName, MetricValue, Packet},
        worker::SerialWorkerController,
    },
};

pub struct Application {
    pub pause_packets: bool,
    pub show_visualization: bool,
    pub connect_the_dots: bool,

    pub serial: SerialWorkerController,

    pub current_time: u32,

    pub packets: AllocRingBuffer<Packet>,
    pub metrics_history: BTreeMap<MetricName, AllocRingBuffer<(u32, MetricValue)>>,

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
                    self.current_time = 0;
                }
                if ui.button("Clear Metrics History").clicked() {
                    self.metrics_history.clear();
                }
                if ui.button("Clear Packets").clicked() {
                    self.packets.clear();
                }
                if ui.button("Clear All").clicked() {
                    self.current_time = 0;
                    self.metrics_history.clear();
                    self.packets.clear();
                }
                ui.checkbox(&mut self.show_visualization, "Show Visualization");

                ui.checkbox(&mut self.pause_packets, "Pause packets");
            });

            ui.heading(format!(
                "Current time: {}",
                metric_timestamp(self.current_time)
            ));
            ui.separator();

            let timestamp_width = Size::exact(7.0 * 10.0);
            let metric_name_width = Size::exact(7.0 * 20.0);
            let metric_type_width = Size::exact(7.0 * 7.0);

            ui.heading("Latest Metrics");
            TableBuilder::new(ui)
                .column(timestamp_width)
                .column(Size::exact(7.0 * 5.0))
                .column(metric_name_width)
                .column(metric_type_width)
                .column(Size::remainder())
                .striped(true)
                .cell_layout(
                    Layout::left_to_right()
                        .with_main_wrap(false)
                        .with_cross_align(eframe::emath::Align::Center),
                )
                .header(20.0, |mut header| {
                    header.col(|ui| {
                        ui.heading("TSLP")
                            .on_hover_text_at_pointer("Time Since Last Packet");
                    });
                    header.col(|ui| {
                        ui.heading("Nr.");
                    });
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
                    for (metric_name, (timestamp, metric_value), count) in
                        self.metrics_history.iter().filter_map(|(name, history)| {
                            history.back().map(|newest| (name, newest, history.len()))
                        })
                    {
                        let is_focusable = metric_value.is_float()
                            || metric_value.is_signed_integer()
                            || metric_value.is_unsigned_integer()
                            || metric_value.is_bool();

                        body.row(20.0, |mut row| {
                            row.col(|ui| {
                                ui.monospace(metric_timestamp(
                                    self.current_time.saturating_sub(*timestamp),
                                ));
                            });
                            row.col(|ui| {
                                ui.monospace(count.to_string());
                            });
                            row.col(|ui| {
                                if is_focusable {
                                    let is_focused = self.focused_metrics.contains(metric_name);

                                    let label = ui
                                        .selectable_label(is_focused, metric_name_text(metric_name))
                                        .on_hover_ui_at_pointer(|ui| {
                                            ui.label(metric_name_text(metric_name));
                                        });
                                    if label.clicked() {
                                        if is_focused {
                                            self.focused_metrics.remove(metric_name);
                                        } else {
                                            self.focused_metrics.insert(metric_name.clone());
                                        }
                                    }
                                } else {
                                    let _ = ui
                                        .selectable_label(false, metric_name_text(metric_name))
                                        .on_hover_ui_at_pointer(|ui| {
                                            ui.label(metric_name_text(metric_name));
                                        });
                                }
                            });
                            row.col(|ui| {
                                let text = RichText::new(metric_value.ty()).monospace().color(
                                    if is_focusable {
                                        Color32::LIGHT_GREEN
                                    } else {
                                        Color32::LIGHT_RED
                                    },
                                );

                                ui.label(text).on_hover_text_at_pointer(if is_focusable {
                                    RichText::new("type can be visualized")
                                        .color(Color32::LIGHT_GREEN)
                                } else {
                                    RichText::new("type can not be visualized")
                                        .color(Color32::LIGHT_RED)
                                });
                            });
                            row.col(|ui| {
                                ui.monospace(metric_value.value())
                                    .on_hover_text_at_pointer(metric_value.value_pretty());
                            });
                        });
                    }
                });

            ui.separator();

            if self.focused_metrics.is_empty() {
                ui.push_id("Packets", |ui| {
                    ui.heading(format!("{} Packets", self.packets.len()));

                    TableBuilder::new(ui)
                        .column(timestamp_width)
                        .column(metric_name_width)
                        .column(metric_type_width)
                        .column(Size::remainder())
                        .striped(true)
                        .cell_layout(Layout::left_to_right().with_main_wrap(false))
                        .header(20.0, |mut header| {
                            header.col(|ui| {
                                ui.heading("Time").on_hover_text_at_pointer(
                                    "Time since the robot has been powered up",
                                );
                            });
                            header.col(|ui| {
                                ui.heading("Name");
                            });
                            header.col(|ui| {
                                ui.heading("Type");
                            });
                            header.col(|ui| {
                                ui.heading("Data");
                            });
                        })
                        .body(|body| {
                            body.rows(15.0, self.packets.len(), |idx, mut row| {
                                let packet = &self.packets[-(idx as isize + 1)];

                                let (timestamp, name, ty, value, value_pretty) = match packet {
                                    Packet::Telemetry(Metric {
                                        timestamp,
                                        value,
                                        name,
                                    }) => (
                                        Some(metric_timestamp(*timestamp)),
                                        metric_name_text(name),
                                        RichText::new(value.ty())
                                            .monospace()
                                            .color(Color32::DARK_GREEN),
                                        RichText::new(value.value()).monospace(),
                                        RichText::new(value.value_pretty()).monospace(),
                                    ),
                                    Packet::System(packet) => (
                                        None,
                                        // TODO: non row element?
                                        RichText::default().into(),
                                        RichText::new("[system]").color(Color32::YELLOW),
                                        RichText::new(format!("{packet:?}"))
                                            .monospace()
                                            .color(Color32::KHAKI),
                                        RichText::new(format!("{packet:#?}"))
                                            .monospace()
                                            .color(Color32::KHAKI),
                                    ),
                                };

                                row.col(|ui| {
                                    ui.monospace(timestamp.unwrap_or_default());
                                });
                                row.col(|ui| {
                                    ui.label(name.clone()).on_hover_ui_at_pointer(|ui| {
                                        ui.label(name);
                                    });
                                });
                                row.col(|ui| {
                                    ui.label(ty.clone()).on_hover_text_at_pointer(ty);
                                });
                                row.col(|ui| {
                                    ui.monospace(value).on_hover_text_at_pointer(value_pretty);
                                });
                            })
                        });
                });
            } else {
                ui.horizontal_wrapped(|ui| {
                    ui.checkbox(&mut self.connect_the_dots, "Connect The Dots?");
                    if ui.button("Clear Focus").clicked() {
                        self.focused_metrics.clear();
                    }
                    ui.label("Focused: ");
                    for focused in &self.focused_metrics {
                        ui.label(metric_name_text(focused));
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

                Plot::new("Focused Metrics")
                    .include_y(0.0)
                    .x_axis_formatter(|x, _range| metric_timestamp(x as u32))
                    .x_grid_spacer(uniform_grid_spacer(|_| [60.0 * 1000.0, 1000.0, 100.0]))
                    .label_formatter(|name, value| {
                        format!(
                            "{name}\n{}\n@ {}",
                            value.y,
                            metric_timestamp(value.x as u32)
                        )
                    })
                    .legend(Legend::default())
                    .show(ui, |ui| {
                        for (metric_name, metric_values) in
                            self.focused_metrics.iter().filter_map(|metric_name| {
                                self.metrics_history
                                    .get(metric_name)
                                    .map(|metric_values| (metric_name, metric_values))
                            })
                        {
                            let values = metric_values
                                .iter()
                                .map(|(timestamp, value)| {
                                    Value::new(
                                        *timestamp as f64,
                                        value
                                            .as_float()
                                            .or_else(|| {
                                                value.as_unsigned_integer().map(|int| int as f64)
                                            })
                                            .or_else(|| {
                                                value.as_signed_integer().map(|int| int as f64)
                                            })
                                            .or_else(|| {
                                                value
                                                    .as_bool()
                                                    .map(|bool| if bool { 1.0 } else { 0.0 })
                                            })
                                            .unwrap_or(f64::NAN),
                                    )
                                })
                                .collect::<Vec<_>>();

                            let color: Color32 = {
                                let mut hasher = DefaultHasher::new();

                                metric_name.hash(&mut hasher);

                                // Get random but deterministic color per line
                                let i = hasher.finish() >> (64 - 5);

                                // Ripped from egui \src\widgets\plot\mod.rs
                                let golden_ratio = (5.0_f32.sqrt() - 1.0) / 2.0; // 0.61803398875
                                let h = i as f32 * golden_ratio;

                                Hsva::new(h, 0.85, 0.5, 1.0).into()
                            };

                            if self.connect_the_dots {
                                ui.line(
                                    Line::new(Values::from_values(values.clone()))
                                        .name(metric_name.to_string())
                                        .color(color),
                                );
                            }
                            ui.points(
                                Points::new(Values::from_values(values))
                                    .radius(2.0)
                                    .name(metric_name.to_string())
                                    .color(color),
                            );
                        }
                    });
            }
        });

        Window::new("Visualization")
            .open(&mut self.show_visualization)
            .frame(egui::Frame::dark_canvas(&ctx.style()))
            .show(ctx, |ui| {
                let (canvas, response) = ui.allocate_exact_size(
                    ui.available_rect_before_wrap().size(),
                    Sense::focusable_noninteractive(),
                );

                // TODO: better (native) canvas coordinates
                let square_dimension = canvas.width().min(canvas.height());

                let robot_rect = Rect::from_center_size(
                    canvas.center(),
                    Vec2::new(square_dimension / 2.0, square_dimension / 2.0),
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

                let heading_length = square_dimension / 4.0 - 15.0;
                let get_heading = |distance: u64, heading: i64| {
                    (heading_length * (distance as f32 / 300.0))
                        * Vec2::angled((heading as f32 + 90.0).to_radians())
                };

                if let Some(front_points) = self
                    .metrics_history
                    .get(&MetricName::namespaced("ultrasonic", "last_readings"))
                    .and_then(|metrics| metrics.back())
                    .and_then(|(_, distance)| distance.as_unsigned_integer_iter())
                {
                    ui.painter().extend(
                        front_points
                            .enumerate()
                            .map(|(heading, distance)| {
                                Shape::circle_filled(
                                    robot_rect.center_top()
                                        - get_heading(distance, heading as i64 - 90),
                                    1.0,
                                    Color32::WHITE,
                                )
                            })
                            .collect(),
                    );
                }

                if let Some((distance, heading)) = Option::zip(
                    self.metrics_history
                        .get(&MetricName::namespaced("ultrasonic", "distance"))
                        .and_then(|metrics| metrics.back())
                        .and_then(|(_, distance)| distance.as_unsigned_integer()),
                    self.metrics_history
                        .get(&MetricName::namespaced("ultrasonic", "heading"))
                        .and_then(|metrics| metrics.back())
                        .and_then(|(_, heading)| heading.as_signed_integer()),
                ) {
                    let ultrasonic_heading = get_heading(distance, heading);

                    let mut shapes = Shape::dashed_line(
                        &[
                            robot_rect.center_top(),
                            robot_rect.center_top() - heading_length * Vec2::Y,
                        ],
                        Stroke::new(4.0, Color32::RED),
                        4.0,
                        8.0,
                    );

                    shapes.push(Shape::line_segment(
                        [
                            robot_rect.center_top(),
                            robot_rect.center_top() - ultrasonic_heading,
                        ],
                        Stroke::new(2.0, Color32::KHAKI),
                    ));

                    shapes.push(Shape::text(
                        &ui.fonts(),
                        robot_rect.center_top() - ultrasonic_heading,
                        Align2::CENTER_BOTTOM,
                        format!("{}cm", distance),
                        FontId::monospace(15.0),
                        Color32::KHAKI,
                    ));

                    shapes.push(Shape::text(
                        &ui.fonts(),
                        robot_rect.center_top() + 15.0 * Vec2::Y,
                        Align2::CENTER_TOP,
                        format!("heading: {heading}°"),
                        FontId::monospace(15.0),
                        Color32::BLUE,
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
                } else {
                    // TODO: warn if missing telemetry?
                }

                if let Some(speed) = self
                    .metrics_history
                    .get(&MetricName::namespaced("motor", "drive_speed"))
                    .and_then(|metrics| metrics.back())
                    .and_then(|(_, speed)| speed.as_float())
                {
                    if speed.abs() > f64::EPSILON {
                        let (color, align, direction) = if speed.is_sign_positive() {
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
