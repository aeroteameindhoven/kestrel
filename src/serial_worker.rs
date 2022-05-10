use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{channel, Receiver, Sender},
        Arc,
    },
    thread::{self, JoinHandle},
};

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_serial::{SerialPortBuilderExt, SerialStream};
use tracing::warn;

pub struct SerialWorker {
    connected: Arc<AtomicBool>,

    packet_rx: Receiver<SerialPacket>,

    handle: JoinHandle<()>,
}

impl SerialWorker {
    pub fn spawn(
        name: String,
        baud_rate: u32,
        repaint: Box<impl Fn() + Send + 'static>,
    ) -> SerialWorker {
        let (packet_tx, packet_rx) = channel();

        let connected = Arc::new(AtomicBool::new(false));

        let handle = thread::Builder::new()
            .name("serial_worker".into())
            .spawn({
                let connected = connected.clone();

                move || serial_worker(name, baud_rate, packet_tx, connected, repaint)
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

    pub fn new_packets(&self) -> impl Iterator<Item = SerialPacket> + '_ {
        self.packet_rx.try_iter()
    }
}

#[derive(Debug)]
pub struct SerialPacket {
    pub name: String,
    pub data: Vec<u8>,
}

#[tokio::main]
async fn serial_worker(
    name: String,
    baud_rate: u32,
    packet_tx: Sender<SerialPacket>,
    connected: Arc<AtomicBool>,
    repaint: Box<impl Fn()>,
) -> ! {
    let serial_port = tokio_serial::new(name, baud_rate)
        .open_native_async()
        .expect("failed to open serial port");

    connected.store(true, Ordering::SeqCst);

    let mut serial_port = BufReader::new(serial_port);
    let mut packet_buffer = Vec::new();

    loop {
        match read_cobs(&mut packet_buffer, &mut serial_port).await {
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

        let name = match read_cobs(&mut packet_buffer, &mut serial_port).await {
            Some(bytes) => String::from_utf8_lossy(bytes).into_owned(),
            None => {
                continue;
            }
        };

        let data = match read_cobs(&mut packet_buffer, &mut serial_port).await {
            Some(bytes) => bytes,
            None => {
                continue;
            }
        };

        packet_tx
            .send(SerialPacket {
                name,
                data: data.to_vec(),
            })
            .expect("ui thread has exited");
        repaint();
    }
}

async fn read_cobs<'vec, 'read>(
    vec: &'vec mut Vec<u8>,
    reader: &'read mut BufReader<SerialStream>,
) -> Option<&'vec [u8]> {
    vec.clear();

    match reader.read_until(0, vec).await {
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
