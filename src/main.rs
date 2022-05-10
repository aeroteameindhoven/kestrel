use std::path::PathBuf;

use argh::FromArgs;
use eframe::{
    egui::{containers::ComboBox, CentralPanel, Context, DragValue, TopBottomPanel},
    App, Frame, NativeOptions,
};
use serial_worker::SerialWorker;

mod serial_worker;

/// Visualization tool for the DBL Venus Exploration project
#[derive(FromArgs, Debug)]
struct Args {
    /// serial port to connect to on startup
    #[argh(positional)]
    port: String,

    /// default baud rate to use
    #[argh(option)]
    baud: Option<u32>,

    /// list the available ports
    #[argh(switch)]
    list: bool,
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt().pretty().init();

    let args: Args = argh::from_env();

    dbg!(&args);

    // serialport::available_ports();

    eframe::run_native(
        env!("CARGO_PKG_NAME"),
        NativeOptions {
            ..Default::default()
        },
        Box::new(move |ctx| {
            let baud = args.baud.unwrap_or(9600);

            Box::new(Application {
                serial: SerialWorker::spawn(
                    serialport::new(args.port, baud)
                        .open()
                        .expect("failed to open serial port"),
                    Box::new({
                        let ctx = ctx.egui_ctx.clone();
                        move || ctx.request_repaint()
                    }),
                ),
            })
        }),
    )
}

struct Application {
    serial: SerialWorker,
}

impl App for Application {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        TopBottomPanel::top("serial_select").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(self.serial.connected().to_string());
            });
        });

        CentralPanel::default().show(ctx, |ui| {});
    }
}
