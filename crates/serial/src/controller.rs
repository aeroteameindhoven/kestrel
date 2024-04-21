use std::{
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, RwLock,
    },
    thread::{self},
};

use kestrel_metric::{Metric, RobotCommand};

use super::{detacher, SerialWorker, SerialWorkerCommand, SerialWorkerState};

pub struct SerialWorkerController {
    port_name: Arc<str>,

    state: Arc<RwLock<SerialWorkerState>>,
    command_tx: Sender<SerialWorkerCommand>,
    metric_rx: Receiver<Metric>,
}

impl SerialWorkerController {
    pub fn spawn(
        port_name: String,
        baud_rate: u32,
        repaint: Box<impl Fn() + Send + 'static>,
    ) -> SerialWorkerController {
        let (metric_tx, metric_rx) = channel();
        let (command_tx, command_rx) = channel();

        let state = Arc::new(RwLock::new(SerialWorkerState::Disconnected));

        let port_name = Arc::from(port_name.into_boxed_str());

        thread::Builder::new()
            .name("serial_worker".into())
            .spawn({
                let state = Arc::clone(&state);
                let port_name = Arc::clone(&port_name);

                move || {
                    SerialWorker {
                        port_name,
                        baud_rate,

                        metric_tx,
                        command_rx,

                        state,

                        repaint,
                    }
                    .spawn()
                }
            })
            .expect("failed to spawn serial worker thread");

        thread::Builder::new()
            .name("serial_detacher".into())
            .spawn({
                let command_tx = command_tx.clone();

                move || detacher::main(command_tx)
            })
            .expect("failed to spawn serial detacher thread");

        Self {
            metric_rx,
            command_tx,

            port_name,
            state,
        }
    }

    pub fn state(&self) -> SerialWorkerState {
        *self.state.read().unwrap()
    }

    pub fn detach(&self) {
        self.command_tx.send(SerialWorkerCommand::Detach).unwrap();
    }

    pub fn attach(&self) {
        self.command_tx.send(SerialWorkerCommand::Attach).unwrap();
    }

    pub fn reset(&self) {
        self.command_tx.send(SerialWorkerCommand::Reset).unwrap();
    }

    pub fn send_command(&self, command: RobotCommand) {
        self.command_tx
            .send(SerialWorkerCommand::SendCommand(command))
            .unwrap();
    }

    pub fn port_name(&self) -> &str {
        self.port_name.as_ref()
    }

    pub fn new_metrics(&self) -> impl Iterator<Item = Metric> + '_ {
        self.metric_rx.try_iter()
    }
}
