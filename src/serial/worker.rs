use std::{
    io::{BufRead, BufReader},
    mem::size_of,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
        Arc,
    },
    thread,
    time::Duration,
};

use serialport::SerialPort;
use time::{ext::NumericalStdDuration, OffsetDateTime};
use tracing::{debug, info, trace, warn};

mod controller;
mod detacher;
mod error;

pub use controller::SerialWorkerController;

use crate::serial::packet::{MetricValue, SystemPacket};

use self::error::{PacketReadError, TransportError};

use super::packet::{Metric, Packet};

struct SerialWorker {
    port_name: Arc<str>,
    baud_rate: u32,
    packet_tx: Sender<(OffsetDateTime, Packet)>,
    connected: Arc<AtomicBool>,
    repaint: Box<dyn Fn()>,

    detach: Arc<AtomicBool>,
}

impl SerialWorker {
    pub fn spawn(mut self) -> ! {
        let mut opt_reader: Option<BufReader<Box<dyn SerialPort>>> = None;
        let mut packet_buffer = Vec::new();

        loop {
            if self.detach.load(Ordering::SeqCst) {
                if opt_reader.is_some() {
                    self.send_packet(Packet::System(SystemPacket::SerialDisconnect));
                }

                opt_reader = None;

                // TODO: lots of repeat calls to keep state up to date, do something about that?
                self.connected.store(false, Ordering::SeqCst);
                self.repaint();

                thread::park();

                continue;
            }

            match &mut opt_reader {
                Some(reader) => match self.read_packet(reader, &mut packet_buffer) {
                    Err(PacketReadError::TransportError(TransportError::TimedOut)) => {}
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

                        thread::sleep(1.std_seconds());
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

    fn connect(&self) -> Option<BufReader<Box<dyn SerialPort>>> {
        match serialport::new(self.port_name.as_ref(), self.baud_rate)
            .timeout(Duration::from_millis(100))
            .open()
        {
            Ok(stream) => Some(BufReader::new(stream)),
            Err(e) if e.kind() == serialport::ErrorKind::NoDevice => None,
            Err(e) => panic!("{e}"),
        }
    }

    fn read_packet(
        &mut self,
        reader: &mut BufReader<Box<dyn SerialPort>>,
        buffer: &mut Vec<u8>,
    ) -> Result<Metric, PacketReadError> {
        let buffer = self.read_cobs(reader, buffer)?;

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
            name: metric_name
                .parse()
                .expect("metric name parsing must never fail"),
            value: metric_value,
        })
    }

    fn read_cobs<'read, 'buffer>(
        &mut self,
        reader: &'read mut BufReader<Box<dyn SerialPort>>,
        buffer: &'buffer mut Vec<u8>,
    ) -> Result<&'buffer [u8], TransportError> {
        buffer.clear();

        let buffer = {
            let len = reader.read_until(0, buffer)?;

            &mut buffer[..len]
        };

        match postcard_cobs::decode_in_place(buffer) {
            Ok(len) => Ok(&buffer[..len.saturating_sub(1)]),
            Err(()) => Err(TransportError::MalformedCOBS(Box::from(&buffer[..]))),
        }
    }
}
