use std::{
    io::{self, BufRead, BufReader, ErrorKind},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{channel, Receiver, Sender, TryRecvError},
        Arc,
    },
    thread::{self, JoinHandle},
};

use serialport::SerialPort;
use tracing::{error, info, warn};

pub struct SerialWorker {
    connected: Arc<AtomicBool>,

    packet_rx: Receiver<SerialPacket>,

    handle: JoinHandle<()>,
}

impl SerialWorker {
    pub fn spawn(
        serial_port: Box<dyn SerialPort>,
        repaint: Box<impl Fn() + Send + 'static>,
    ) -> SerialWorker {
        let (packet_tx, packet_rx) = channel();

        let connected = Arc::new(AtomicBool::new(true));

        let handle = thread::Builder::new()
            .name("serial_worker".into())
            .spawn({
                let connected = connected.clone();

                move || serial_worker(serial_port, packet_tx, connected, repaint)
            })
            .expect("failed to spawn serial worker thread");

        Self {
            packet_rx,

            handle,

            connected,
        }
    }

    pub fn connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }
}

pub struct SerialPacket {
    pub name: String,
    pub data: Vec<u8>,
}

fn serial_worker(
    serial_port: Box<dyn SerialPort>,
    packet_tx: Sender<SerialPacket>,
    connected: Arc<AtomicBool>,
    repaint: Box<impl Fn()>,
) -> ! {
    let mut serial_port = BufReader::new(serial_port);
    let mut packet_buffer = Vec::new();

    loop {
        match read_cobs(&mut packet_buffer, &mut serial_port) {
            Some([0xDE, 0xAD, 0xBE, 0xEF]) => {}
            Some(data) => {
                warn!(
                    ?data,
                    "Expected header packet, got something else. Sender de-sync?"
                );

                continue;
            }
            None => continue,
        }

        let name = match read_cobs(&mut packet_buffer, &mut serial_port) {
            Some(bytes) => String::from_utf8_lossy(bytes).into_owned(),
            None => {
                continue;
            }
        };

        let data = match read_cobs(&mut packet_buffer, &mut serial_port) {
            Some(bytes) => bytes,
            None => {
                continue;
            }
        };

        info!(?name, ?data, "Received packet");
    }
}

fn read_cobs<'vec, 'read>(
    vec: &'vec mut Vec<u8>,
    reader: &'read mut dyn BufRead,
) -> Option<&'vec [u8]> {
    vec.clear();

    match reader.read_until(0, vec) {
        Ok(_) => match postcard_cobs::decode_in_place(vec) {
            Ok(len) => Some(&vec[..len.saturating_sub(1)]),
            Err(()) => {
                warn!("received malformed COBS data");
                None
            }
        },
        Err(error) => panic!("{error} {}", error.kind()),
    }
}
