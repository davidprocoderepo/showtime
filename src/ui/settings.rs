//! Configurações do app: frame rate, offset, dispositivo MIDI, rede TCP.

use crate::timecode::Timecode;

/// Estado das configurações (editado na janela de Configurações).
#[derive(Debug, Clone)]
pub struct AppSettings {
    pub fps: f64,
    pub drop_frame: bool,
    pub offset: Timecode,
    pub midi_device: String,
    pub tcp_ip: String,
    pub tcp_port: u16,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            fps: 30.0,
            drop_frame: false,
            offset: Timecode::ZERO,
            midi_device: String::new(),
            tcp_ip: "192.168.1.10".to_string(),
            tcp_port: 3000,
        }
    }
}

impl AppSettings {
    /// Renderiza os campos de configuração. Retorna `true` se algo mudou
    /// (ex.: fps/offset) — o app então recalcula os timecodes dos marcadores.
    pub fn show(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;

        ui.heading("Configurações");
        ui.add_space(6.0);

        egui::ComboBox::from_label("Frame rate")
            .selected_text(format!("{:.2} fps", self.fps))
            .show_ui(ui, |ui| {
                for fps in [24.0, 25.0, 30.0, 29.97] {
                    let selected = (self.fps - fps).abs() < f64::EPSILON;
                    if ui.selectable_label(selected, format!("{fps} fps")).clicked() {
                        self.fps = fps;
                        if fps != 29.97 {
                            self.drop_frame = false;
                        }
                        changed = true;
                    }
                }
            });

        let mut drop = self.drop_frame;
        if ui
            .add_enabled(
                (self.fps - 29.97).abs() < 0.001,
                egui::Checkbox::new(&mut drop, "Drop-frame (29.97)"),
            )
            .changed()
        {
            self.drop_frame = drop;
            changed = true;
        }

        ui.add_space(6.0);
        ui.label("Offset (HH:MM:SS:FF):");
        let mut offset_str = self.offset.to_string();
        let offset_resp = ui.add(
            egui::TextEdit::singleline(&mut offset_str).hint_text("00:00:00:00"),
        );
        if offset_resp.lost_focus()
            && let Ok(tc) = offset_str.parse::<Timecode>()
            && tc != self.offset
        {
            self.offset = tc;
            changed = true;
            // Entrada inválida ou inalterada: mantém o offset anterior.
        }

        ui.add_space(12.0);
        ui.separator();
        ui.heading("MIDI");
        ui.label("Dispositivo de saída (vazio = primeiro):");
        changed |= ui
            .add(egui::TextEdit::singleline(&mut self.midi_device).hint_text("Ex.: IAC Driver"))
            .changed();

        ui.add_space(12.0);
        ui.separator();
        ui.heading("Rede GrandMA2");
        ui.label("Protocolo não-oficial; o console precisa aceitar comandos por rede.");
        changed |= ui
            .add(egui::TextEdit::singleline(&mut self.tcp_ip).hint_text("192.168.1.10"))
            .changed();
        let mut port = self.tcp_port as i64;
        if ui
            .add(egui::DragValue::new(&mut port).range(1..=65535).prefix("Porta: "))
            .changed()
        {
            self.tcp_port = port as u16;
            changed = true;
        }

        changed
    }
}