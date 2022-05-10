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

use serial2::SerialPort;
use tracing::{error, info, warn};

pub struct SerialWorker {
    port_path: Option<PathBuf>,
    connected: Arc<AtomicBool>,

    command_tx: Sender<SerialWorkerCommand>,
    packet_rx: Receiver<SerialPacket>,

    handle: JoinHandle<()>,
}

enum SerialWorkerCommand {
    Connect { name: PathBuf, baud: u32 },
    Disconnect,
}

impl SerialWorker {
    pub fn spawn(repaint: Box<impl Fn() + Send + 'static>) -> SerialWorker {
        let (command_tx, command_rx) = channel();
        let (packet_tx, packet_rx) = channel();

        let connected = Arc::new(AtomicBool::new(false));

        let handle = thread::Builder::new()
            .name("serial_worker".into())
            .spawn({
                let connected = connected.clone();

                move || serial_worker(command_rx, packet_tx, connected, repaint)
            })
            .expect("failed to spawn serial worker thread");

        Self {
            command_tx,
            packet_rx,

            handle,

            port_path: None,
            connected,
        }
    }

    pub fn connected_port(&self) -> Option<&PathBuf> {
        self.connected
            .load(Ordering::SeqCst)
            .then(|| self.port_path.as_ref())
            .flatten()
    }

    pub fn connect(&mut self, port: PathBuf, baud: u32) {
        self.port_path = Some(port.clone());
        self.command_tx
            .send(SerialWorkerCommand::Connect { name: port, baud })
            .expect("serial worker thread has exited");
    }
}

pub struct SerialPacket {
    pub name: String,
    pub data: Vec<u8>,
}

fn serial_worker(
    command_rx: Receiver<SerialWorkerCommand>,
    packet_tx: Sender<SerialPacket>,
    connected: Arc<AtomicBool>,
    repaint: Box<impl Fn()>,
) -> ! {
    let mut serial_port = None;
    let mut packet_buffer = Vec::new();

    loop {
        match command_rx.try_recv() {
            Err(TryRecvError::Disconnected) => panic!("ui thread has existed"),
            Err(TryRecvError::Empty) => {}
            Ok(SerialWorkerCommand::Connect { name, baud }) => {
                info!(?name, "Connecting to serial port");

                match SerialPort::open(&name, baud) {
                    Ok(port) => {
                        serial_port = Some(BufReader::new(port));
                        info!(?name, "Connected to serial port");
                        connected.store(true, Ordering::SeqCst);
                    }
                    Err(e) => error!(?e, ?name, "failed to open serial port"),
                }
            }
            Ok(SerialWorkerCommand::Disconnect) => {
                serial_port.take();
            }
        }

        if let Some(serial_port) = &mut serial_port {
            match read_cobs(&mut packet_buffer, serial_port) {
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

            let name = match read_cobs(&mut packet_buffer, serial_port) {
                Some(bytes) => String::from_utf8_lossy(bytes).into_owned(),
                None => {
                    continue;
                }
            };

            let data = match read_cobs(&mut packet_buffer, serial_port) {
                Some(bytes) => bytes,
                None => {
                    continue;
                }
            };

            info!(?name, ?data, "Received packet");
        }
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
