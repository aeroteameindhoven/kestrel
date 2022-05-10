use std::path::PathBuf;

use eframe::{
    egui::{containers::ComboBox, CentralPanel, Context, DragValue, TopBottomPanel},
    App, Frame, NativeOptions,
};
use serial2::SerialPort;
use serial_worker::SerialWorker;
use argh::FromArgs;

mod serial_worker;

/// Visualization tool for the DBL Venus Exploration project
#[derive(FromArgs)]
struct Args {
    /// serial port to connect to on startup
    #[argh(positional)]
    port: Option<PathBuf>,
    /// default baud rate to use
    #[argh(option)]
    baud: Option<u32>,
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt().pretty().init();

    let args: Args = argh::from_env();

    eframe::run_native(
        env!("CARGO_PKG_NAME"),
        NativeOptions {
            ..Default::default()
        },
        Box::new(move |ctx| {
            let mut serial = SerialWorker::spawn(Box::new({
                let ctx = ctx.egui_ctx.clone();
                move || ctx.request_repaint()
            }));

            let default_baud = args.baud.unwrap_or(9600);

            if let Some(port) = args.port {
                serial.connect(port, default_baud);
            }

            Box::new(Application {
                serial_ports: SerialPort::available_ports().unwrap(),
                baud_rate: default_baud,
                serial,
            })
        }),
    )
}

struct Application {
    serial_ports: Vec<PathBuf>,
    baud_rate: u32,
    serial: SerialWorker,
}

impl App for Application {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        TopBottomPanel::top("serial_select").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.add(DragValue::new(&mut self.baud_rate).suffix(" baud"));

                if ui.button("Refresh").clicked() {
                    // TODO: do this not in ui thread maybe?
                    self.serial_ports = SerialPort::available_ports().unwrap();
                }

                ComboBox::new("serial_port_selector", "UART port")
                    .selected_text(
                        self.serial
                            .connected_port()
                            .map(|selected_port| selected_port.to_string_lossy().into_owned())
                            .unwrap_or_else(|| "none".into()),
                    )
                    .show_ui(ui, |ui| {
                        for port in &self.serial_ports {
                            if ui
                                .selectable_label(
                                    self.serial.connected_port() == Some(port),
                                    port.to_string_lossy().into_owned(),
                                )
                                .clicked()
                            {
                                self.serial.connect(port.clone(), self.baud_rate);
                            }
                        }
                    });
            });
        });

        CentralPanel::default().show(ctx, |ui| {

        });
    }
}
