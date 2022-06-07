use std::collections::BTreeSet;

use eframe::{
    egui::{Layout, RichText, Ui},
    epaint::Color32,
};
use egui_extras::{Size, TableBuilder};

use crate::serial::metric::{name::MetricName, timestamp::Timestamp, value::MetricValue};

use super::sizes::{METRIC_NAME_WIDTH, METRIC_TYPE_WIDTH, MONOSPACE_CHAR_WIDTH, TIMESTAMP_WIDTH};

pub fn latest_metrics<'ui, 'metric>(
    ui: &'ui mut Ui,
    current_time: Timestamp,
    focused_metrics: &mut BTreeSet<MetricName>,
    latest_metrics: impl Iterator<
        Item = (
            &'metric MetricName,
            &'metric (Timestamp, MetricValue),
            usize,
        ),
    >,
) {
    TableBuilder::new(ui)
        .column(Size::exact(TIMESTAMP_WIDTH))
        .column(Size::exact(MONOSPACE_CHAR_WIDTH * 5.0))
        .column(Size::exact(METRIC_NAME_WIDTH))
        .column(Size::exact(METRIC_TYPE_WIDTH))
        .column(Size::remainder())
        .striped(true)
        .cell_layout(
            Layout::left_to_right()
                .with_main_wrap(false)
                .with_cross_align(eframe::emath::Align::Center),
        )
        .header(20.0, |mut header| {
            header.col(|ui| {
                ui.heading("TSLM")
                    .on_hover_text_at_pointer("Time Since Latest Metric");
            });
            header.col(|ui| {
                ui.heading("Cnt").on_hover_text_at_pointer("Metric Count");
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
            for (metric_name, (timestamp, metric_value), count) in latest_metrics {
                let is_focusable = metric_value.is_float()
                    || metric_value.is_signed_integer()
                    || metric_value.is_unsigned_integer()
                    || metric_value.is_bool();

                body.row(20.0, |mut row| {
                    row.col(|ui| {
                        ui.monospace((current_time - *timestamp).to_string());
                    });
                    row.col(|ui| {
                        ui.monospace(count.to_string());
                    });
                    row.col(|ui| {
                        if is_focusable {
                            let is_focused = focused_metrics.contains(metric_name);

                            let label = ui
                                .selectable_label(is_focused, metric_name)
                                .on_hover_ui_at_pointer(|ui| {
                                    ui.label(metric_name);
                                });
                            if label.clicked() {
                                if is_focused {
                                    focused_metrics.remove(metric_name);
                                } else {
                                    focused_metrics.insert(metric_name.clone());
                                }
                            }
                        } else {
                            let _ = ui
                                .selectable_label(false, metric_name)
                                .on_hover_ui_at_pointer(|ui| {
                                    ui.label(metric_name);
                                });
                        }
                    });
                    row.col(|ui| {
                        let text =
                            RichText::new(metric_value.ty())
                                .monospace()
                                .color(if is_focusable {
                                    Color32::LIGHT_GREEN
                                } else {
                                    Color32::LIGHT_RED
                                });

                        ui.label(text).on_hover_text_at_pointer(if is_focusable {
                            RichText::new("type can be focused").color(Color32::LIGHT_GREEN)
                        } else {
                            RichText::new("type can not be focused").color(Color32::LIGHT_RED)
                        });
                    });
                    row.col(|ui| {
                        ui.monospace(metric_value.value())
                            .on_hover_text_at_pointer(metric_value.value_pretty());
                    });
                });
            }
        });
}
