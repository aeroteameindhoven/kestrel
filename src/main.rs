use std::collections::{BTreeMap, BTreeSet};

use app::Application;
use argh::FromArgs;
use eframe::NativeOptions;
use ringbuffer::AllocRingBuffer;
use kestrel_metric::timestamp::Timestamp;
use kestrel_serial::SerialWorkerController;
use tracing::info;
use tracing_subscriber::filter::LevelFilter;

use crate::version::GIT_VERSION;

mod app;
mod version;
mod visualization;

/// Visualization tool for the DBL Venus Exploration project
#[derive(FromArgs, Debug)]
struct Args {
    /// serial port to connect to on startup
    #[argh(positional)]
    port: Option<String>,

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
        .with_ansi(cfg!(debug_assertions))
        .init();

    info!(version = GIT_VERSION);

    let args: Args = argh::from_env();

    if args.list {
        // TODO:
        dbg!(serialport::available_ports()?);

        return Ok(());
    }

    let baud = args.baud.unwrap_or(115200);
    let port = if let Some(port) = args.port {
        port
    } else {
        serialport::available_ports()?
            .first()
            .expect("no serial port available")
            .port_name
            .clone()
    };

    eframe::run_native(
        env!("CARGO_PKG_NAME"),
        NativeOptions {
            ..Default::default()
        },
        Box::new(move |ctx| {
            Box::new(Application {
                pause_metrics: false,
                show_visualization: false,
                show_info: false,
                connect_the_dots: true,

                raw_metrics: new_metric_ring_buffer(),
                sorted_metrics: BTreeMap::new(),

                current_time: Timestamp::default(),

                focused_metrics: BTreeSet::new(),
                hidden_metrics: BTreeSet::new(),

                serial: SerialWorkerController::spawn(
                    port,
                    baud,
                    Box::new({
                        let ctx = ctx.egui_ctx.clone();

                        move || ctx.request_repaint()
                    }),
                ),
            })
        }),
    ).unwrap(); // FIXME: not Send or Sync :/ color eyre does no like it

    Ok(())
}

pub fn new_metric_ring_buffer<T>() -> AllocRingBuffer<T> {
    AllocRingBuffer::new(1024)
}
