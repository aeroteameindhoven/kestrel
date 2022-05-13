use std::{
    io::Read,
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
};

use tracing::{error, warn};

pub fn main(detach: Arc<AtomicBool>, handle: Arc<JoinHandle<()>>) {
    let listener = TcpListener::bind("127.0.0.1:6969").expect("failed to bind tcp listener");

    let buf = &mut [0u8; 6];

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => match stream.read_exact(buf) {
                Ok(()) => match &buf[..] {
                    b"attach" => {
                        detach.store(false, Ordering::SeqCst);
                        handle.thread().unpark();
                    }
                    b"detach" => {
                        detach.store(true, Ordering::SeqCst);
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
