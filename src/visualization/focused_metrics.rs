use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use eframe::{
    egui::{
        plot::{uniform_grid_spacer, Legend, Line, Plot, Points, Value, Values},
        Ui,
    },
    epaint::{color::Hsva, Color32},
};

use crate::serial::packet::{
    metric_name::MetricName, metric_value::MetricValue, timestamp::Timestamp,
};

fn label_formatter(name: &str, value: &Value) -> String {
    format!(
        "{name}\n{}\n@ {}",
        value.y,
        Timestamp::from_millis(value.x as u32)
    )
}

fn color_from_metric_name(metric_name: &MetricName) -> Color32 {
    let mut hasher = DefaultHasher::new();

    metric_name.hash(&mut hasher);

    // Get random but deterministic color per line
    let i = hasher.finish() >> (64 - 4);

    // Ripped from egui \src\widgets\plot\mod.rs
    let golden_ratio = (5.0_f32.sqrt() - 1.0) / 2.0; // 0.61803398875
    let h = i as f32 * golden_ratio;

    Hsva::new(h, 0.85, 0.5, 1.0).into()
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
    Plot::new("Focused Metrics")
        .include_y(0.0)
        .x_axis_formatter(|x, _range| Timestamp::from_millis(x as u32).to_string())
        .x_grid_spacer(uniform_grid_spacer(|_| [60.0 * 1000.0, 1000.0, 100.0]))
        .label_formatter(label_formatter)
        .legend(Legend::default())
        .show(ui, |ui| {
            for (metric_name, metric_values) in focused_metrics {
                let values = metric_values
                    .map(|(timestamp, value)| {
                        Value::new(
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
