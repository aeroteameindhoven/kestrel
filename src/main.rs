use std::collections::{BTreeMap, BTreeSet};

use app::Application;
use argh::FromArgs;
use eframe::{egui::CentralPanel, NativeOptions};
use kestrel_metric::timestamp::Timestamp;
use kestrel_serial::SerialWorkerController;
use ringbuffer::AllocRingBuffer;
use tracing::info;
use tracing_subscriber::EnvFilter;

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
        .with_env_filter(EnvFilter::from_default_env())
        .compact()
        .with_ansi(cfg!(debug_assertions))
        .init();

    info!(version = GIT_VERSION);

    let args: Args = argh::from_env();

    let serial_ports = || {
        serialport::available_ports().map(|ports| {
            ports
                .into_iter()
                .filter(|port| port.port_type != serialport::SerialPortType::Unknown)
        })
    };

    if args.list {
        // TODO:
        dbg!(serial_ports()?);

        return Ok(());
    }

    let baud = args.baud.unwrap_or(115200);
    let port = if let Some(port) = args.port {
        port
    } else {
        serial_ports()?
            .next()
            .expect("no serial port available")
            .port_name
            .clone()
    };

    let serial_ports = serial_ports()?.collect::<Vec<_>>();

    let mut fonts = eframe::egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);

    eframe::run_simple_native(
        env!("CARGO_PKG_NAME"),
        Default::default(),
        move |ctx, frame| {
            ctx.set_fonts(fonts.clone()); // FIXME: this should be in setup

            CentralPanel::default().show(ctx, |ui| {
                for port in &serial_ports {
                    ui.horizontal(|ui| {
                        ui.label(&port.port_name);

                        match &port.port_type {
                            serialport::SerialPortType::UsbPort(info) => {
                                ui.label(egui_phosphor::regular::USB);

                                if let Some(product) = &info.product {
                                    ui.label(product);
                                }

                                if let Some(manufacture) = &info.manufacturer {
                                    ui.label(manufacture);
                                }

                                if let Some(serial_number) = &info.serial_number {
                                    ui.label(format!("({serial_number})"));
                                }

                                ui.label(format!("[{}:{}]", info.vid, info.pid));
                            }
                            serialport::SerialPortType::PciPort => {
                                ui.label(egui_phosphor::regular::CPU);
                            }
                            serialport::SerialPortType::BluetoothPort => {
                                ui.label(egui_phosphor::regular::BLUETOOTH);
                            }
                            serialport::SerialPortType::Unknown => {
                                ui.label("?");
                            }
                        }
                    });
                }
            });
        },
    )
    .unwrap();

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
    )
    .unwrap(); // FIXME: not Send or Sync :/ color eyre does no like it

    Ok(())
}

pub fn new_metric_ring_buffer<T>() -> AllocRingBuffer<T> {
    AllocRingBuffer::new(1024)
}
