use eframe::{
    egui::{Layout, RichText, Ui},
    epaint::Color32,
};
use egui_extras::{Size, TableBuilder};
use ringbuffer::{AllocRingBuffer, RingBuffer};

use crate::serial::metric::Metric;

use super::sizes::{METRIC_NAME_WIDTH, METRIC_TYPE_WIDTH, TIMESTAMP_WIDTH};

pub fn metrics_history(ui: &mut Ui, metrics: &AllocRingBuffer<Metric>) {
    ui.push_id("metrics_history", |ui| {
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
                body.rows(15.0, metrics.len(), |idx, mut row| {
                    let metric = &metrics[-(idx as isize + 1)];

                    row.col(|ui| {
                        ui.monospace(metric.timestamp.to_string());
                    });
                    row.col(|ui| {
                        ui.label(&metric.name).on_hover_ui_at_pointer(|ui| {
                            ui.label(&metric.name);
                        });
                    });
                    row.col(|ui| {
                        let ty = RichText::new(metric.value.ty())
                            .monospace()
                            .color(Color32::DARK_GREEN);

                        ui.label(ty.clone()).on_hover_text_at_pointer(ty);
                    });
                    row.col(|ui| {
                        ui.monospace(RichText::new(metric.value.value()).monospace())
                            .on_hover_text_at_pointer(
                                RichText::new(metric.value.value_pretty()).monospace(),
                            );
                    });
                })
            });
    });
}
