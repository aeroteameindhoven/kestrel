use std::{
    any::Any,
    ffi::CStr,
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
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
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

#[derive(Debug, Clone)]
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
    F32(f32),
    F64(f64),
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

impl From<io::Error> for TransportError {
    fn from(error: io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::TimedOut => TransportError::SerialPortDisconnected,
            _ => panic!("encountered IO error: {error}"),
        }
    }
}

enum PacketReadError {
    PoorLayout { section: usize, packet: Box<[u8]> },
    BadPacketLength { expected: Option<usize>, got: usize },
    BadValueLength { expected: usize, got: usize },
    TransportError(TransportError),
}

impl From<TransportError> for PacketReadError {
    fn from(e: TransportError) -> Self {
        Self::TransportError(e)
    }
}

impl From<io::Error> for PacketReadError {
    fn from(error: io::Error) -> Self {
        Self::TransportError(error.into())
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
                        warn!(?data, "Received malformed COBS data");
                    }
                    Err(PacketReadError::BadValueLength { expected, got }) => {
                        debug!(%expected, %got, "Value did not match expected length");
                    }
                    Err(PacketReadError::BadPacketLength { expected, got }) => {
                        debug!(
                            ?expected,
                            %got,
                            "Packet length did not match expected length"
                        );
                    }
                    Err(PacketReadError::PoorLayout { packet, section }) => {
                        warn!(?packet, %section, "Received packet with a bad layout");
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
        let buffer = self.read_cobs(reader, buffer).await?;

        let (packet, packet_length) = buffer.split_at(buffer.len().saturating_sub(2));

        let packet_length =
            packet_length
                .try_into()
                .map_err(|_| PacketReadError::BadPacketLength {
                    expected: None,
                    got: packet.len(),
                })?;
        let packet_length = u16::from_le_bytes(packet_length) as usize - size_of::<u16>();

        if packet_length != packet.len() {
            return Err(PacketReadError::BadPacketLength {
                expected: Some(packet_length),
                got: packet.len(),
            });
        }

        let mut split = packet.splitn(3, |&b| b == 0x00);

        let metric_name = split.next().ok_or_else(|| PacketReadError::PoorLayout {
            section: 0,
            packet: Box::from(packet),
        })?;
        let metric_name = String::from_utf8_lossy(metric_name).into_owned();

        let metric_type = split.next().ok_or_else(|| PacketReadError::PoorLayout {
            section: 1,
            packet: Box::from(packet),
        })?;
        let metric_type = String::from_utf8_lossy(metric_type).into_owned();

        let metric = split.next().ok_or_else(|| PacketReadError::PoorLayout {
            section: 2,
            packet: Box::from(packet),
        })?;

        macro_rules! metric {
            (as $ty:ty) => {
                <$ty>::from_le_bytes(metric.try_into().map_err(|_| {
                    PacketReadError::BadValueLength {
                        expected: size_of::<$ty>(),
                        got: metric.len(),
                    }
                })?)
            };
        }

        let metric_value = match metric_type.as_str() {
            "u8" => MetricValue::U8(metric!(as u8)),
            "u16" => MetricValue::U16(metric!(as u16)),
            "u32" => MetricValue::U32(metric!(as u32)),
            "u64" => MetricValue::U64(metric!(as u64)),

            "i8" => MetricValue::I8(metric!(as i8)),
            "i16" => MetricValue::I16(metric!(as i16)),
            "i32" => MetricValue::I32(metric!(as i32)),
            "i64" => MetricValue::I64(metric!(as i64)),

            "bool" => MetricValue::Bool(metric!(as u8) != 0),

            "f32" => MetricValue::F32(metric!(as f32)),
            "f64" => MetricValue::F64(metric!(as f64)),

            _ => {
                warn!(%metric_type, "received metric of unknown type");

                MetricValue::Unknown(metric_type, Box::from(metric))
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

        let buffer = {
            let len = reader.read_until(0, buffer).await?;

            &mut buffer[..len]
        };

        match postcard_cobs::decode_in_place(buffer) {
            Ok(len) => Ok(&buffer[..len.saturating_sub(1)]),
            Err(()) => Err(TransportError::MalformedCOBS(Box::from(&buffer[..]))),
        }
    }
}
