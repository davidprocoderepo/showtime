//! Controles de transporte: play/pause/stop, seek e volume.
//!
//! Mostra a barra de status: posição, timecode atual, duração, frame rate e
//! drop-frame.

use crate::timecode::Timecode;

/// Ações pedidas pela barra de transporte.
#[derive(Debug, Default)]
pub struct TransportAction {
    pub play: bool,
    pub pause: bool,
    pub stop: bool,
    pub seek_to: Option<f64>,
    pub volume: Option<f32>,
}

/// Dados exibidos pela barra (posição já avançada pelo relógio do playback).
#[derive(Debug, Clone)]
pub struct TransportData<'a> {
    pub position_sec: f64,
    pub duration_sec: f64,
    pub is_playing: bool,
    pub volume: f32,
    pub current_timecode: &'a Timecode,
    pub fps: f64,
    pub drop_frame: bool,
    pub has_audio: bool,
}

/// Renderiza a barra de transporte.
pub fn show(ui: &mut egui::Ui, data: &mut TransportData) -> TransportAction {
    let mut action = TransportAction::default();

    ui.horizontal(|ui| {
        ui.add_enabled_ui(!data.is_playing && data.has_audio, |ui| {
            if ui.button("▶ Play").clicked() {
                action.play = true;
            }
        });
        ui.add_enabled_ui(data.is_playing, |ui| {
            if ui.button("⏸ Pausar").clicked() {
                action.pause = true;
            }
        });
        ui.add_enabled_ui(data.has_audio, |ui| {
            if ui.button("⏹ Parar").clicked() {
                action.stop = true;
            }
        });

        ui.separator();

        let mut slider_pos = data.position_sec;
        let slider = ui.add(
            egui::Slider::new(&mut slider_pos, 0.0..=data.duration_sec.max(0.001))
                .show_value(false)
                .custom_formatter(|v, _| format!("{:.1}s", v)),
        );
        if slider.changed() && (slider_pos - data.position_sec).abs() > 1e-4 {
            action.seek_to = Some(slider_pos);
        }

        ui.separator();

        ui.label(
            egui::RichText::new(data.current_timecode.to_string())
                .monospace()
                .strong(),
        );
        ui.label(format!(
            "{:.2}s / {:.2}s",
            data.position_sec, data.duration_sec
        ));
        ui.label(format!(
            "{}fps{}",
            data.fps,
            if data.drop_frame { " DF" } else { "" }
        ));

        ui.separator();

        let mut vol = data.volume;
        if ui
            .add(egui::Slider::new(&mut vol, 0.0..=1.0).text("Volume"))
            .changed()
        {
            action.volume = Some(vol);
        }
    });

    action
}