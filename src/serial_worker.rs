use std::{
    any::Any,
    io,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{channel, Receiver, Sender},
        Arc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_serial::{SerialPortBuilderExt, SerialStream};
use tracing::{info, warn, trace, debug};

pub struct SerialWorkerController {
    port_name: Arc<str>,

    connected: Arc<AtomicBool>,
    packet_rx: Receiver<SerialPacket>,

    handle: JoinHandle<()>,
}

impl SerialWorkerController {
    pub fn spawn(
        port_name: String,
        baud_rate: u32,
        repaint: Box<impl Fn() + Send + 'static>,
    ) -> SerialWorkerController {
        let (packet_tx, packet_rx) = channel();

        let connected = Arc::new(AtomicBool::new(false));
        let port_name = Arc::from(port_name.into_boxed_str());

        let handle = thread::Builder::new()
            .name("serial_worker".into())
            .spawn({
                let connected = Arc::clone(&connected);
                let port_name = Arc::clone(&port_name);

                move || {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_time()
                        .build()
                        .unwrap();

                    runtime.block_on(
                        SerialWorker {
                            port_name,
                            baud_rate,
                            packet_tx,
                            connected,
                            repaint,

                            packet_buffer: Vec::new(),
                        }
                        .spawn(),
                    )
                }
            })
            .expect("failed to spawn serial worker thread");

        Self {
            packet_rx,
            handle,

            port_name,

            connected,
        }
    }

    pub fn connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    pub fn port_name(&self) -> &str {
        self.port_name.as_ref()
    }

    pub fn new_packets(&self) -> impl Iterator<Item = SerialPacket> + '_ {
        self.packet_rx.try_iter()
    }

    pub fn join(self) -> Result<(), Box<dyn Any + Send + 'static>> {
        self.handle.join()
    }
}

#[derive(Debug)]
pub struct SerialPacket {
    pub name: String,
    pub data: Vec<u8>,
}

struct SerialWorker {
    port_name: Arc<str>,
    baud_rate: u32,
    packet_tx: Sender<SerialPacket>,
    connected: Arc<AtomicBool>,
    repaint: Box<dyn Fn()>,

    packet_buffer: Vec<u8>,
}

enum COBSReadError {
    SerialPortDisconnected,
    MalformedCOBS,
}

impl SerialWorker {
    pub async fn spawn(mut self) -> ! {
        let mut opt_reader = None;

        loop {
            match &mut opt_reader {
                Some(reader) => match self.read_packet(reader).await {
                    Err(COBSReadError::SerialPortDisconnected) => {
                        info!("serial port disconnected");

                        opt_reader = None;

                        self.connected.store(true, Ordering::SeqCst);
                        self.repaint();
                    }
                    Err(COBSReadError::MalformedCOBS) => {
                        warn!("received malformed COBS data");
                    }
                    Ok(Some(packet)) => {
                        self.packet_tx.send(packet).expect("ui thread has exited");
                        self.repaint();
                    }
                    Ok(None) => {
                        debug!("failed to read packet");
                    }
                },
                None => match self.connect() {
                    Some(reader) => {
                        info!("serial port connected");

                        opt_reader = Some(reader);

                        self.connected.store(true, Ordering::SeqCst);
                        self.repaint();
                    }
                    None => {
                        trace!("serial port not found... sleeping 1 second");

                        tokio::time::sleep(Duration::from_millis(1000)).await;
                    }
                },
            }
        }
    }

    fn repaint(&self) {
        (self.repaint)()
    }

    fn connect(&self) -> Option<BufReader<SerialStream>> {
        match tokio_serial::new(self.port_name.as_ref(), self.baud_rate).open_native_async() {
            Ok(stream) => Some(BufReader::new(stream)),
            Err(e) if e.kind() == tokio_serial::ErrorKind::NoDevice => None,
            Err(e) => panic!("{e}"),
        }
    }

    async fn read_packet<'read>(
        &mut self,
        reader: &'read mut BufReader<SerialStream>,
    ) -> Result<Option<SerialPacket>, COBSReadError> {
        match self.read_cobs(reader).await? {
            [0xDE, 0xAD, 0xBE, 0xEF] => {}
            data => {
                warn!(
                    ?data,
                    "Expected header packet, got something else. Sender de-sync?"
                );

                return Ok(None);
            }
        }

        let name = String::from_utf8_lossy(self.read_cobs(reader).await?).into_owned();

        let data = self.read_cobs(reader).await?;

        Ok(Some(SerialPacket {
            name,
            data: data.to_vec(),
        }))
    }

    async fn read_cobs<'read>(
        &mut self,
        reader: &'read mut BufReader<SerialStream>,
    ) -> Result<&[u8], COBSReadError> {
        self.packet_buffer.clear();

        match reader.read_until(0, &mut self.packet_buffer).await {
            Ok(_) => match postcard_cobs::decode_in_place(&mut self.packet_buffer) {
                Ok(len) => Ok(&self.packet_buffer[..len.saturating_sub(1)]),
                Err(()) => Err(COBSReadError::MalformedCOBS),
            },
            Err(error) if error.kind() == io::ErrorKind::TimedOut => {
                info!("serial port disconnected");
                self.connected.store(false, Ordering::SeqCst);

                Err(COBSReadError::SerialPortDisconnected)
            }
            Err(error) => panic!("{error} {}", error.kind()),
        }
    }
}
