use eframe::{
    egui::{CentralPanel, Context, TopBottomPanel},
    App, Frame,
};

use crate::serial_worker::{SerialPacket, SerialWorkerController};

pub struct Application {
    pub serial: SerialWorkerController,
    pub packets: Vec<SerialPacket>,
}

impl App for Application {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        self.packets.extend(self.serial.new_packets());

        TopBottomPanel::top("serial_select").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("Serial port {}", self.serial.port_name(),));
                if !self.serial.connected() {
                    ui.label("connecting");

                    ui.spinner();
                } else {
                    ui.label("connected");
                }
            });
        });

        CentralPanel::default().show(ctx, |ui| {
            for packet in &self.packets {
                ui.label(format!("{packet:?}"));
                ui.separator();
            }
        });
    }
}
