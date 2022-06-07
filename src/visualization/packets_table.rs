use eframe::{
    egui::{Layout, RichText, Ui, WidgetText},
    epaint::Color32,
};
use egui_extras::{Size, TableBuilder};
use ringbuffer::{AllocRingBuffer, RingBuffer};

use crate::serial::packet::{Metric, Packet};

use super::sizes::{METRIC_NAME_WIDTH, METRIC_TYPE_WIDTH, TIMESTAMP_WIDTH};

pub fn packets_table(ui: &mut Ui, packets: &AllocRingBuffer<Packet>) {
    ui.push_id("Packets", |ui| {
        TableBuilder::new(ui)
            .column(Size::exact(TIMESTAMP_WIDTH))
            .column(Size::exact(METRIC_NAME_WIDTH))
            .column(Size::exact(METRIC_TYPE_WIDTH))
            .column(Size::remainder())
            .striped(true)
            .cell_layout(Layout::left_to_right().with_main_wrap(false))
            .header(20.0, |mut header| {
                header.col(|ui| {
                    ui.heading("Time")
                        .on_hover_text_at_pointer("Time since the robot has been powered up");
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
                body.rows(15.0, packets.len(), |idx, mut row| {
                    let packet = &packets[-(idx as isize + 1)];

                    let (timestamp, name, ty, value, value_pretty) = match packet {
                        Packet::Telemetry(Metric {
                            timestamp,
                            value,
                            name,
                        }) => (
                            Some(timestamp.to_string()),
                            WidgetText::from(name),
                            RichText::new(value.ty())
                                .monospace()
                                .color(Color32::DARK_GREEN),
                            RichText::new(value.value()).monospace(),
                            RichText::new(value.value_pretty()).monospace(),
                        ),
                        Packet::System(packet) => (
                            None,
                            // TODO: non row element?
                            WidgetText::default(),
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
}
