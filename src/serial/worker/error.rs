use std::io;

pub enum TransportError {
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

pub enum PacketReadError {
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
