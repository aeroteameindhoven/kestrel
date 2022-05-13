use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{channel, Receiver},
        Arc,
    },
    thread::{self, JoinHandle},
};

use time::OffsetDateTime;

use crate::serial::packet::Packet;

use super::{detacher, SerialWorker};

pub struct SerialWorkerController {
    port_name: Arc<str>,

    connected: Arc<AtomicBool>,
    detach: Arc<AtomicBool>,
    packet_rx: Receiver<(OffsetDateTime, Packet)>,

    handle: Arc<JoinHandle<()>>,
}

impl SerialWorkerController {
    pub fn spawn(
        port_name: String,
        baud_rate: u32,
        repaint: Box<impl Fn() + Send + 'static>,
    ) -> SerialWorkerController {
        let (packet_tx, packet_rx) = channel();

        let connected = Arc::new(AtomicBool::new(false));
        let detach = Arc::new(AtomicBool::new(false));

        let port_name = Arc::from(port_name.into_boxed_str());

        let handle = Arc::new(
            thread::Builder::new()
                .name("serial_worker".into())
                .spawn({
                    let connected = Arc::clone(&connected);
                    let detach = Arc::clone(&detach);
                    let port_name = Arc::clone(&port_name);

                    move || {
                        SerialWorker {
                            port_name,
                            baud_rate,
                            packet_tx,
                            connected,
                            repaint,

                            detach,
                        }
                        .spawn()
                    }
                })
                .expect("failed to spawn serial worker thread"),
        );

        thread::Builder::new()
            .name("serial_detacher".into())
            .spawn({
                let handle = Arc::clone(&handle);
                let detach = Arc::clone(&detach);

                move || detacher::main(detach, handle)
            })
            .expect("failed to spawn serial detacher thread");

        Self {
            packet_rx,
            handle,

            port_name,

            connected,
            detach,
        }
    }

    pub fn connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    pub fn detached(&self) -> bool {
        self.detach.load(Ordering::SeqCst)
    }

    pub fn detach(&self) {
        self.detach.store(true, Ordering::SeqCst);
    }

    pub fn attach(&self) {
        self.detach.store(false, Ordering::SeqCst);
        self.handle.thread().unpark();
    }

    pub fn port_name(&self) -> &str {
        self.port_name.as_ref()
    }

    pub fn new_packets(&self) -> impl Iterator<Item = (OffsetDateTime, Packet)> + '_ {
        self.packet_rx.try_iter()
    }
}
