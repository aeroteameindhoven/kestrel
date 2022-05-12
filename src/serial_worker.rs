use std::{
    any::Any,
    io,
    mem::size_of,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{channel, Receiver, Sender},
        Arc,
    },
    thread::{self, JoinHandle},
};

use time::{ext::NumericalStdDuration, OffsetDateTime};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_serial::{SerialPortBuilderExt, SerialStream};
use tracing::{debug, info, trace, warn};

pub struct SerialWorkerController {
    port_name: Arc<str>,

    connected: Arc<AtomicBool>,
    packet_rx: Receiver<(OffsetDateTime, Packet)>,

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
                        .enable_io()
                        .build()
                        .unwrap();

                    runtime.block_on(
                        SerialWorker {
                            port_name,
                            baud_rate,
                            packet_tx,
                            connected,
                            repaint,
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

    pub fn new_packets(&self) -> impl Iterator<Item = (OffsetDateTime, Packet)> + '_ {
        self.packet_rx.try_iter()
    }

    pub fn join(self) -> Result<(), Box<dyn Any + Send + 'static>> {
        self.handle.join()
    }
}

#[derive(Debug)]
pub enum Packet {
    Telemetry(Metric),
    System(SystemPacket),
}

#[derive(Debug)]
pub struct Metric {
    pub name: String,
    pub value: MetricValue,
}

#[derive(Debug)]
pub enum MetricValue {
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    Bool(bool),
    Unknown(String, Box<[u8]>),
}

#[derive(Debug)]
pub enum SystemPacket {
    SerialDisconnect,
    SerialConnect,
}

struct SerialWorker {
    port_name: Arc<str>,
    baud_rate: u32,
    packet_tx: Sender<(OffsetDateTime, Packet)>,
    connected: Arc<AtomicBool>,
    repaint: Box<dyn Fn()>,
}

enum TransportError {
    SerialPortDisconnected,
    MalformedCOBS(Box<[u8]>),
}

enum PacketReadError {
    BadHeader(Box<[u8]>),
    BadValueLength { expected: usize, got: usize },
    TransportError(TransportError),
}

impl From<TransportError> for PacketReadError {
    fn from(e: TransportError) -> Self {
        Self::TransportError(e)
    }
}

impl SerialWorker {
    pub async fn spawn(mut self) -> ! {
        let mut opt_reader = None;
        let mut packet_buffer = Vec::new();

        loop {
            match &mut opt_reader {
                Some(reader) => match self.read_packet(reader, &mut packet_buffer).await {
                    Err(PacketReadError::TransportError(
                        TransportError::SerialPortDisconnected,
                    )) => {
                        info!("serial port disconnected");

                        opt_reader = None;

                        self.connected.store(false, Ordering::SeqCst);
                        self.send_packet(Packet::System(SystemPacket::SerialDisconnect));
                        self.repaint();
                    }
                    Err(PacketReadError::TransportError(TransportError::MalformedCOBS(data))) => {
                        warn!(?data, "received malformed COBS data");
                    }
                    Err(PacketReadError::BadHeader(header)) => {
                        debug!(
                            ?header,
                            "Expected header, got something else. Sender de-sync?"
                        );
                    }
                    Err(PacketReadError::BadValueLength { expected, got }) => {
                        debug!(%expected, %got, "Value did not match expected length");
                    }
                    Ok(metric) => {
                        self.send_packet(Packet::Telemetry(metric));
                        self.repaint();
                    }
                },
                None => match self.connect() {
                    Some(reader) => {
                        info!("serial port connected");

                        opt_reader = Some(reader);

                        self.connected.store(true, Ordering::SeqCst);
                        self.send_packet(Packet::System(SystemPacket::SerialConnect));
                        self.repaint();
                    }
                    None => {
                        trace!("serial port not found... sleeping 1 second");

                        tokio::time::sleep(1.std_seconds()).await;
                    }
                },
            }
        }
    }

    fn send_packet(&self, packet: Packet) {
        self.packet_tx
            .send((OffsetDateTime::now_utc(), packet))
            .expect("ui thread has exited");
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

    async fn read_packet<'read, 'buffer>(
        &mut self,
        reader: &'read mut BufReader<SerialStream>,
        buffer: &'buffer mut Vec<u8>,
    ) -> Result<Metric, PacketReadError> {
        // Read ""header"" (just 4 empty COBS frames)
        for _ in 0..4 {
            let header = self.read_cobs(reader, buffer).await?;

            if !header.is_empty() {
                return Err(PacketReadError::BadHeader(Box::from(header)));
            }
        }

        let metric_name =
            String::from_utf8_lossy(self.read_cobs(reader, buffer).await?).into_owned();
        let metric_type =
            String::from_utf8_lossy(self.read_cobs(reader, buffer).await?).into_owned();

        let data = self.read_cobs(reader, buffer).await?;

        macro_rules! metric {
            ($data:ident as $ty:ty) => {
                <$ty>::from_le_bytes($data.try_into().map_err(|_| {
                    PacketReadError::BadValueLength {
                        expected: size_of::<$ty>(),
                        got: data.len(),
                    }
                })?)
            };
        }

        let metric_value = match metric_type.as_str() {
            "u8" => MetricValue::U8(metric!(data as u8)),
            "u16" => MetricValue::U16(metric!(data as u16)),
            "u32" => MetricValue::U32(metric!(data as u32)),
            "u64" => MetricValue::U64(metric!(data as u64)),

            "i8" => MetricValue::I8(metric!(data as i8)),
            "i16" => MetricValue::I16(metric!(data as i16)),
            "i32" => MetricValue::I32(metric!(data as i32)),
            "i64" => MetricValue::I64(metric!(data as i64)),

            "bool" => MetricValue::Bool(metric!(data as u8) != 0),

            _ => {
                warn!(%metric_type, "received metric of unknown type");

                MetricValue::Unknown(metric_type, Box::from(data))
            }
        };

        Ok(Metric {
            name: metric_name,
            value: metric_value,
        })
    }

    async fn read_cobs<'read, 'buffer>(
        &mut self,
        reader: &'read mut BufReader<SerialStream>,
        buffer: &'buffer mut Vec<u8>,
    ) -> Result<&'buffer [u8], TransportError> {
        buffer.clear();

        match reader.read_until(0, buffer).await {
            Ok(len) => match postcard_cobs::decode_in_place(buffer) {
                Ok(len) => Ok(&buffer[..len.saturating_sub(1)]),
                Err(()) => Err(TransportError::MalformedCOBS(Box::from(
                    &buffer[..len.saturating_sub(1)],
                ))),
            },
            Err(error) if error.kind() == io::ErrorKind::TimedOut => {
                info!("serial port disconnected");
                self.connected.store(false, Ordering::SeqCst);

                Err(TransportError::SerialPortDisconnected)
            }
            Err(error) => panic!("{error} {}", error.kind()),
        }
    }
}
