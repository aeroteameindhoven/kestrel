use std::{io::Read, net::TcpListener, sync::mpsc::Sender};

use tracing::{error, warn};

use super::SerialWorkerCommand;

// TODO: move this into the app
pub(super) fn main(command_tx: Sender<SerialWorkerCommand>) {
    let listener = TcpListener::bind("127.0.0.1:6969").expect("failed to bind tcp listener");

    let buf = &mut [0u8; 6];

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => match stream.read_exact(buf) {
                Ok(()) => match &buf[..] {
                    b"attach" => {
                        command_tx.send(SerialWorkerCommand::Attach).unwrap();
                    }
                    b"detach" => {
                        command_tx.send(SerialWorkerCommand::Detach).unwrap();
                    }
                    _ => {
                        warn!("received non-recognized data over tcp connection");
                    }
                },
                Err(err) => error!(?err, "encountered an error reading from tcp connection"),
            },
            Err(err) => error!(?err, "failed to accept incoming tcp connection"),
        }
    }
}
