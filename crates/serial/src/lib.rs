use std::{
    io::{BufRead, BufReader},
    mem::size_of,
    sync::{
        mpsc::{Receiver, Sender},
        Arc, RwLock,
    },
    thread,
    time::Duration,
};

use serialport::SerialPort;
use tracing::{debug, error, info, trace, warn};

mod controller;
mod detacher;
mod error;

pub use controller::SerialWorkerController;

use kestrel_metric::{
    timestamp::Timestamp,
    value::{MetricValue, MetricValueError},
    Metric, RobotCommand,
};

use self::error::{PacketReadError, TransportError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SerialWorkerCommand {
    Detach,
    Attach,
    Reset,
    SendCommand(RobotCommand),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerialWorkerState {
    Resetting,
    Connected,
    Disconnected,
    Detached,
}

struct SerialWorker {
    port_name: Arc<str>,
    baud_rate: u32,
    metric_tx: Sender<Metric>,
    command_rx: Receiver<SerialWorkerCommand>,
    state: Arc<RwLock<SerialWorkerState>>,
    repaint: Box<dyn Fn()>,
}

impl SerialWorker {
    pub fn spawn(mut self) -> ! {
        let mut opt_reader: Option<BufReader<Box<dyn SerialPort>>> = None;
        let mut packet_buffer = Vec::new();

        loop {
            for command in self.command_rx.try_iter() {
                match command {
                    SerialWorkerCommand::Detach => {
                        opt_reader.take();

                        info!("serial worker detached");
                        *self.state.write().unwrap() = SerialWorkerState::Detached;
                        self.repaint();

                        loop {
                            let command = self.command_rx.recv().unwrap();

                            // Wait for an attach command
                            match command {
                                SerialWorkerCommand::Attach => break,
                                _ => info!(?command, "ignoring command while detached"),
                            }
                        }

                        info!("serial worker attached");
                        *self.state.write().unwrap() = SerialWorkerState::Disconnected;
                        self.repaint();
                    }
                    SerialWorkerCommand::Attach => {
                        warn!("serial worker commanded to attach when already attached");
                    }
                    SerialWorkerCommand::Reset => match &mut opt_reader {
                        Some(reader) => {
                            let serial = reader.get_mut();

                            *self.state.write().unwrap() = SerialWorkerState::Resetting;

                            serial.write_data_terminal_ready(true).unwrap();
                            thread::sleep(Duration::from_millis(1000));
                            serial.write_data_terminal_ready(false).unwrap();

                            *self.state.write().unwrap() = SerialWorkerState::Connected;
                        }
                        None => warn!(
                            "serial worker commanded to reset when not connected to an arduino"
                        ),
                    },
                    SerialWorkerCommand::SendCommand(command) => {
                        match &mut opt_reader {
                            Some(reader) => {
                                let serial = reader.get_mut();
                                serial.write_all(&[command as u8]).unwrap();
                                serial.flush().unwrap();
                            }
                            None => warn!(
                                "serial worker commanded to send command when not connected to an arduino"
                            ),
                        }
                    }
                }
            }

            match &mut opt_reader {
                Some(reader) => match self.read_packet(reader, &mut packet_buffer) {
                    Err(PacketReadError::Transport(TransportError::TimedOut)) => {}
                    Err(PacketReadError::Transport(TransportError::SerialPortDisconnected)) => {
                        info!("serial port disconnected");

                        opt_reader = None;

                        *self.state.write().unwrap() = SerialWorkerState::Disconnected;
                        self.repaint();
                    }
                    Err(PacketReadError::Transport(TransportError::MalformedCOBS(data))) => {
                        warn!(?data, "Received malformed COBS data");
                    }
                    Err(PacketReadError::MetricValue(MetricValueError::BadLength {
                        expected,
                        got,
                    })) => {
                        error!(%expected, %got, "Metric value did not match expected length");
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
                        self.metric_tx.send(metric).expect("ui thread has exited");
                        self.repaint();
                    }
                },
                None => match self.connect() {
                    Some(reader) => {
                        info!("serial port connected");

                        opt_reader = Some(reader);

                        *self.state.write().unwrap() = SerialWorkerState::Connected;
                        self.repaint();
                    }
                    None => {
                        trace!("serial port not found... sleeping 1 second");

                        thread::sleep(Duration::from_millis(1000));
                    }
                },
            }
        }
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

        let packet = {
            let (packet, packet_length) = buffer.split_at(buffer.len().saturating_sub(2));

            let packet_length =
                packet_length
                    .try_into()
                    .map_err(|_| PacketReadError::BadPacketLength {
                        expected: None,
                        got: packet.len(),
                    })?;
            let packet_length =
                (u16::from_le_bytes(packet_length) as usize).saturating_sub(size_of::<u16>());

            if packet_length != packet.len() {
                dbg!(buffer);

                return dbg!(Err(PacketReadError::BadPacketLength {
                    expected: Some(packet_length),
                    got: packet.len(),
                }));
            }

            packet
        };

        let (packet, timestamp) = {
            // Should never panic since packet length has been verified
            let (timestamp, packet) = packet.split_at(size_of::<u32>());

            let timestamp = u32::from_le_bytes(
                timestamp
                    .try_into()
                    .expect("timestamp should always be one u32 wide"),
            );

            (packet, timestamp)
        };

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

        let metric_value = MetricValue::from_bytes(metric_type, metric)?;

        Ok(Metric {
            timestamp: Timestamp::from_millis(timestamp),
            name: metric_name
                .parse()
                .expect("metric name parsing must never fail"),
            value: metric_value,
        })
    }

    fn read_cobs<'buffer>(
        &mut self,
        reader: &mut BufReader<Box<dyn SerialPort>>,
        buffer: &'buffer mut Vec<u8>,
    ) -> Result<&'buffer [u8], TransportError> {
        buffer.clear();

        let buffer = {
            let len = reader.read_until(0, buffer)?;

            &mut buffer[..len]
        };

        match postcard_cobs::decode_in_place(buffer) {
            Ok(len) => Ok(&buffer[..len.saturating_sub(1)]),
            Err(()) => Err(TransportError::MalformedCOBS(Box::from(&*buffer))),
        }
    }
}
