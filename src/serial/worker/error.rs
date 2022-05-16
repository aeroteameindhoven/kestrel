use std::io;

use crate::serial::packet::MetricValueError;

pub enum TransportError {
    TimedOut,
    SerialPortDisconnected,
    MalformedCOBS(Box<[u8]>),
}

impl From<io::Error> for TransportError {
    fn from(error: io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::TimedOut => TransportError::TimedOut,
            io::ErrorKind::PermissionDenied => TransportError::SerialPortDisconnected,
            kind => panic!("encountered IO error: {error} {kind:?}"),
        }
    }
}

pub enum PacketReadError {
    PoorLayout { section: usize, packet: Box<[u8]> },
    BadPacketLength { expected: Option<usize>, got: usize },

    MetricValue(MetricValueError),
    Transport(TransportError),
}

impl From<TransportError> for PacketReadError {
    fn from(error: TransportError) -> Self {
        Self::Transport(error)
    }
}

impl From<io::Error> for PacketReadError {
    fn from(error: io::Error) -> Self {
        Self::Transport(error.into())
    }
}

impl From<MetricValueError> for PacketReadError {
    fn from(error: MetricValueError) -> Self {
        Self::MetricValue(error)
    }
}
