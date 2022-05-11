use eframe::{
    egui::{CentralPanel, Context, RichText, TopBottomPanel},
    epaint::Color32,
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
                ui.label(format!("Serial port {}", self.serial.port_name()));

                ui.separator();

                if !self.serial.connected() {
                    ui.label(
                        RichText::new("Waiting for serial port to become available")
                            .color(Color32::YELLOW),
                    );

                    ui.spinner();
                } else {
                    ui.label(RichText::new("Connected").color(Color32::GREEN));
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
