use app::Application;
use argh::FromArgs;
use eframe::NativeOptions;
use serial_worker::SerialWorkerController;
use tracing_subscriber::filter::LevelFilter;

mod app;
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
    tracing_subscriber::fmt()
        .with_max_level(LevelFilter::TRACE)
        .compact()
        .init();

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
                serial: SerialWorkerController::spawn(
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
