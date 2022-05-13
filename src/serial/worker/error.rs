use std::io;

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
