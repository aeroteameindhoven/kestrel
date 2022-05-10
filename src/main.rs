use argh::FromArgs;
use eframe::{
    egui::{CentralPanel, Context, TopBottomPanel},
    App, Frame, NativeOptions,
};
use serial_worker::{SerialWorker, SerialPacket};

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

    if args.list {
        // TODO:
        dbg!(tokio_serial::available_ports()?);

        return Ok(());
    }

    let baud = args.baud.unwrap_or(9600);

    eframe::run_native(
        env!("CARGO_PKG_NAME"),
        NativeOptions {
            ..Default::default()
        },
        Box::new(move |ctx| {
            Box::new(Application {
                packets: Vec::new(),
                serial: SerialWorker::spawn(
                    args.port,
                    baud,
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
    packets: Vec<SerialPacket>,
}

impl App for Application {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        self.packets.extend(self.serial.new_packets());

        TopBottomPanel::top("serial_select").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(self.serial.connected().to_string());
            });
        });

        CentralPanel::default().show(ctx, |ui| {
            for packet in &self.packets {
                ui.label(format!("{packet:?}"));
                ui.separator();
            }
        });
    }
}
