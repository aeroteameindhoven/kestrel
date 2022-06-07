use eframe::{
    egui::{Sense, Ui},
    emath::{Align2, Rect, Vec2},
    epaint::{Color32, FontId, Shape, Stroke},
};

use crate::serial::packet::{metric_name::MetricName, metric_value::MetricValue};

pub fn robot<'ui, 'metric>(
    ui: &'ui mut Ui,
    get_latest_value: impl Fn(MetricName) -> Option<&'metric MetricValue>,
) {
    let (canvas, _response) = ui.allocate_exact_size(
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

    if let Some(front_points) =
        get_latest_value(MetricName::namespaced("ultrasonic", "last_readings"))
            .and_then(|distance| distance.as_unsigned_integer_iter())
    {
        ui.painter().extend(
            front_points
                .enumerate()
                .map(|(heading, distance)| {
                    Shape::circle_filled(
                        robot_rect.center_top() - get_heading(distance, heading as i64 - 90),
                        1.0,
                        Color32::WHITE,
                    )
                })
                .collect(),
        );
    }

    if let Some((distance, heading)) = Option::zip(
        get_latest_value(MetricName::namespaced("ultrasonic", "distance"))
            .and_then(|distance| distance.as_unsigned_integer()),
        get_latest_value(MetricName::namespaced("ultrasonic", "heading"))
            .and_then(|heading| heading.as_signed_integer()),
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

    if let Some(speed) = get_latest_value(MetricName::namespaced("motor", "drive_speed"))
        .and_then(|speed| speed.as_float())
    {
        if speed.abs() > f64::EPSILON {
            let (color, align, direction) = if speed.is_sign_positive() {
                (Color32::RED, Align2::CENTER_BOTTOM, Vec2::Y * -1.0)
            } else {
                (Color32::BLUE, Align2::CENTER_TOP, Vec2::Y)
            };

            let arrow_base = robot_rect.center() + 13.0 * direction;
            let arrow_tip =
                arrow_base + direction * ((robot_rect.height() / 4.0 * speed.abs() as f32) - 13.0);

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
}
