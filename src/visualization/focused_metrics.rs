use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use eframe::{egui::Ui, epaint::Color32};
use egui_plot::{uniform_grid_spacer, Corner, Legend, Line, Plot, PlotPoint, PlotPoints, Points};
use kestrel_metric::{name::MetricName, timestamp::Timestamp, value::MetricValue};

fn label_formatter(name: &str, value: &PlotPoint) -> String {
    format!("{name}\n{}\n@ {}", value.y, x_value_formatter(value.x))
}

fn x_value_formatter(value: f64) -> String {
    format!(
        "{}{}",
        if value.is_sign_negative() { "-" } else { "" },
        Timestamp::from_millis(value.abs() as u32)
    )
}

fn color_from_metric_name(metric_name: &MetricName) -> Color32 {
    let mut hasher = DefaultHasher::new();

    metric_name.hash(&mut hasher);

    // Get random but deterministic color per line
    let index = hasher.finish();

    let color = colorous::RAINBOW.eval_rational(index as usize, u64::MAX as usize);

    Color32::from_rgb(color.r, color.g, color.b)
}

pub fn focused_metrics_plot<'ui, 'iter>(
    ui: &'ui mut Ui,
    focused_metrics: impl Iterator<
            Item = (
                &'iter MetricName,
                impl Iterator<Item = &'iter (Timestamp, MetricValue)>,
            ),
        > + 'iter,
    connect_the_dots: bool,
) {
    Plot::new("focused_metrics")
        .include_y(0.0)
        .include_y(1.0)
        .x_axis_formatter(|grid_mark, chars, _range| {
            // FIXME: assert!(chars >= 8, "Need to implement shrinkage");

            x_value_formatter(grid_mark.value)
        })
        .x_grid_spacer(uniform_grid_spacer(|_| [60.0 * 1000.0, 1000.0, 100.0]))
        .label_formatter(label_formatter)
        .legend(Legend::default().position(Corner::LeftTop))
        .show(ui, |ui| {
            for (metric_name, metric_values) in focused_metrics {
                let values = metric_values
                    .map(|(timestamp, value)| {
                        PlotPoint::new(
                            timestamp.timestamp(),
                            value
                                .as_float()
                                .or_else(|| value.as_unsigned_integer().map(|int| int as f64))
                                .or_else(|| value.as_signed_integer().map(|int| int as f64))
                                .or_else(|| {
                                    value.as_bool().map(|bool| if bool { 1.0 } else { 0.0 })
                                })
                                .unwrap_or(f64::NAN),
                        )
                    })
                    .collect::<Vec<_>>();

                let color = color_from_metric_name(metric_name);

                if connect_the_dots {
                    ui.line(
                        Line::new(PlotPoints::Owned(values.clone()))
                            .name(metric_name.to_string())
                            .color(color),
                    );
                }
                ui.points(
                    Points::new(PlotPoints::Owned(values))
                        .radius(2.0)
                        .name(metric_name.to_string())
                        .color(color),
                );
            }
        });
}
