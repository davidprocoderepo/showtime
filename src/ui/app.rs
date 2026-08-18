//! Struct principal do eframe::App: janela, menus, orquestração do core.
//!
//! A UI é em português. Decodificação de áudio roda em thread de fundo
//! (nunca bloquear a thread da UI); a waveform é calculada na própria thread
//! e entregue junto.

use std::path::PathBuf;
use std::sync::{mpsc, Arc};

use crate::audio::decoder;
use crate::audio::playback::Playback;
use crate::audio::waveform::{compute_peaks, Waveform};
use crate::error::ShowtimeError;
use crate::export;
use crate::live::midi_events::{MidiEventConfig, MidiEventSender};
use crate::live::mtc::{MtcConfig, MtcSender};
use crate::live::tcp_client::{Ma2TcpClient, TcpConfig};
use crate::markers::manager::MarkerManager;
use crate::markers::model::MarkerType;
use crate::project::io::{load_json, load_yaml, save_json, save_yaml};
use crate::project::Project;
use crate::timecode::{seconds_to_timecode, Timecode};

use super::marker_panel::{self, MarkerEditDraft, MarkerEditResult};
use super::settings::AppSettings;
use super::timeline::{self, TimelineInput, TimelineState};
use super::transport::{self, TransportData};

/// Áudio carregado em memória (samples compartilhados via Arc).
struct LoadedAudio {
    samples: Arc<Vec<f32>>,
    sample_rate: u32,
    channels: u16,
    duration_sec: f64,
}

/// Formato de exportação disponível no menu.
#[derive(Debug, Clone, Copy)]
enum ExportKind {
    Csv,
    Xml,
    Ma2Macro,
    Midi,
}

/// Aplicação principal.
pub struct ShowtimeApp {
    // Projeto e marcadores.
    project_name: String,
    project_path: Option<PathBuf>,
    audio_path: Option<String>,
    manager: MarkerManager,

    // Áudio.
    sample_rate: u32,
    channels: u16,
    duration_sec: f64,
    waveform: Option<Waveform>,
    playback: Option<Playback>,
    decode_rx: Option<mpsc::Receiver<Result<(LoadedAudio, Waveform), ShowtimeError>>>,
    decoding: bool,

    // Estado da UI.
    error: Option<String>,
    show_settings: bool,
    selected_marker: Option<u32>,
    editing: Option<MarkerEditDraft>,
    timeline: TimelineState,
    volume: f32,

    // Configurações.
    settings: AppSettings,

    // Modo ao vivo.
    mtc: Option<MtcSender>,
    mtc_enabled: bool,
    midi: Option<MidiEventSender>,
    midi_enabled: bool,
    tcp: Option<Ma2TcpClient>,
    last_pos: f64,
}

impl ShowtimeApp {
    pub fn new() -> Self {
        ShowtimeApp {
            project_name: "Novo projeto".to_string(),
            project_path: None,
            audio_path: None,
            manager: MarkerManager::new(),
            sample_rate: 44100,
            channels: 2,
            duration_sec: 0.0,
            waveform: None,
            playback: None,
            decode_rx: None,
            decoding: false,
            error: None,
            show_settings: false,
            selected_marker: None,
            editing: None,
            timeline: TimelineState::default(),
            volume: 0.8,
            settings: AppSettings::default(),
            mtc: None,
            mtc_enabled: false,
            midi: None,
            midi_enabled: false,
            tcp: None,
            last_pos: 0.0,
        }
    }

    // ---------------------------------------------------------------- áudio

    /// Carrega o áudio em thread de fundo (decode + waveform).
    fn load_audio(&mut self, path: PathBuf) {
        let (tx, rx) = mpsc::channel();
        self.decode_rx = Some(rx);
        self.decoding = true;
        self.audio_path = Some(path.display().to_string());
        std::thread::spawn(move || {
            let res = decoder::decode_file(&path).map(|a| {
                let w = compute_peaks(&a.samples, 1024, a.channels);
                (
                    LoadedAudio {
                        samples: Arc::new(a.samples),
                        sample_rate: a.sample_rate,
                        channels: a.channels,
                        duration_sec: a.duration_sec,
                    },
                    w,
                )
            });
            let _ = tx.send(res);
        });
    }

    /// Consome o resultado do decode em background (chamado a cada frame).
    fn poll_decode(&mut self) {
        let Some(rx) = self.decode_rx.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(Ok((loaded, w))) => {
                self.decoding = false;
                self.playback = Some(Playback::new(
                    Arc::clone(&loaded.samples),
                    loaded.sample_rate,
                    loaded.channels,
                ));
                self.sample_rate = loaded.sample_rate;
                self.channels = loaded.channels;
                self.duration_sec = loaded.duration_sec;
                self.waveform = Some(w);
            }
            Ok(Err(e)) => {
                self.decoding = false;
                self.error = Some(e.to_string());
            }
            Err(mpsc::TryRecvError::Empty) => {
                self.decode_rx = Some(rx);
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.decoding = false;
            }
        }
    }

    fn current_position(&mut self) -> f64 {
        self.playback.as_mut().map_or(0.0, |p| p.position_sec())
    }

    fn current_timecode(&self, sec: f64) -> Timecode {
        seconds_to_timecode(
            sec,
            self.settings.fps,
            self.settings.drop_frame,
            self.settings.offset,
        )
        .unwrap_or(Timecode::ZERO)
    }

    // ------------------------------------------------------------- marcadores

    fn add_marker_at(&mut self, sec: f64) {
        let tc = self.current_timecode(sec);
        let cue = self
            .manager
            .markers()
            .iter()
            .map(|m| m.cue_number)
            .max()
            .unwrap_or(0)
            + 1;
        let id = self
            .manager
            .add("Marcador", sec, tc, cue, 1, MarkerType::Go);
        self.selected_marker = Some(id);
        if let Some(m) = self.manager.get(id) {
            self.editing = Some(MarkerEditDraft::from_marker(m));
        }
    }

    /// Recalcula o timecode de todos os marcadores (após mudar fps/offset).
    fn recompute_marker_timecodes(&mut self) {
        let fps = self.settings.fps;
        let drop = self.settings.drop_frame;
        let offset = self.settings.offset;
        let ids: Vec<u32> = self.manager.markers().iter().map(|m| m.id).collect();
        for id in ids {
            self.manager.update(id, |m| {
                if let Ok(tc) = seconds_to_timecode(m.time_sec, fps, drop, offset) {
                    m.timecode = tc;
                }
            });
        }
    }

    // --------------------------------------------------------------- projetos

    fn build_project(&self) -> Project {
        Project {
            name: self.project_name.clone(),
            audio_file_path: self.audio_path.clone(),
            frame_rate: self.settings.fps,
            drop_frame: self.settings.drop_frame,
            timecode_offset: self.settings.offset,
            markers: self.manager.markers().to_vec(),
        }
    }

    fn new_project(&mut self) {
        self.project_name = "Novo projeto".to_string();
        self.project_path = None;
        self.audio_path = None;
        self.manager = MarkerManager::new();
        self.playback = None;
        self.waveform = None;
        self.duration_sec = 0.0;
        self.selected_marker = None;
        self.editing = None;
    }

    fn save_project(&mut self, save_as: bool) {
        let path = if save_as || self.project_path.is_none() {
            rfd::FileDialog::new()
                .add_filter("Projeto Showtime", &["json", "yaml", "yml"])
                .set_file_name("projeto.json")
                .save_file()
        } else {
            self.project_path.clone()
        };
        if let Some(p) = path {
            let proj = self.build_project();
            let ext = p
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("json")
                .to_lowercase();
            let res = if ext == "yaml" || ext == "yml" {
                save_yaml(&proj, &p)
            } else {
                save_json(&proj, &p)
            };
            match res {
                Ok(()) => {
                    self.project_path = Some(p);
                }
                Err(e) => self.error = Some(e.to_string()),
            }
        }
    }

    fn open_project(&mut self) {
        let Some(p) = rfd::FileDialog::new()
            .add_filter("Projeto Showtime", &["json", "yaml", "yml"])
            .pick_file()
        else {
            return;
        };
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("json")
            .to_lowercase();
        let res = if ext == "yaml" || ext == "yml" {
            load_yaml(&p)
        } else {
            load_json(&p)
        };
        match res {
            Ok(proj) => self.apply_project(proj, p),
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn apply_project(&mut self, proj: Project, path: PathBuf) {
        self.project_name = proj.name.clone();
        self.audio_path = proj.audio_file_path.clone();
        self.settings.fps = proj.frame_rate;
        self.settings.drop_frame = proj.drop_frame;
        self.settings.offset = proj.timecode_offset;
        self.manager = MarkerManager::with_markers(proj.markers);
        self.project_path = Some(path);
        if let Some(ap) = &proj.audio_file_path {
            let p = PathBuf::from(ap);
            if p.exists() {
                self.load_audio(p);
            } else {
                self.error = Some(format!("Arquivo de áudio não encontrado: {ap}"));
            }
        }
    }

    // --------------------------------------------------------------- exports

    fn export_file(&mut self, kind: ExportKind) {
        if self.manager.markers().is_empty() {
            self.error = Some("Nenhum marcador para exportar".into());
            return;
        }
        let (filter, ext, name) = match kind {
            ExportKind::Csv => ("CSV", "csv", "cues.csv"),
            ExportKind::Xml => ("XML", "xml", "cues.xml"),
            ExportKind::Ma2Macro => ("Macro MA2", "xml", "showtime_macro.xml"),
            ExportKind::Midi => ("MIDI", "mid", "cues.mid"),
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter(filter, &[ext])
            .set_file_name(name)
            .save_file()
        else {
            return;
        };
        let res = match kind {
            ExportKind::Csv => std::fs::write(&path, export::csv::to_csv(self.manager.markers()))
                .map_err(ShowtimeError::from),
            ExportKind::Xml => std::fs::write(&path, export::xml::to_xml(self.manager.markers()))
                .map_err(ShowtimeError::from),
            ExportKind::Ma2Macro => std::fs::write(
                &path,
                export::ma2_script::to_ma2_macro_xml("Showtime Cues", self.manager.markers()),
            )
            .map_err(ShowtimeError::from),
            ExportKind::Midi => export::midi_file::to_midi_bytes(self.manager.markers())
                .and_then(|bytes| std::fs::write(&path, bytes).map_err(ShowtimeError::from)),
        };
        if let Err(e) = res {
            self.error = Some(e.to_string());
        }
    }

    // ---------------------------------------------------------------- ao vivo

    fn toggle_mtc(&mut self) {
        if self.mtc_enabled {
            let device = self.settings.midi_device.trim().to_string();
            let config = MtcConfig {
                device_name: if device.is_empty() { None } else { Some(device) },
                fps: self.settings.fps,
                drop_frame: self.settings.drop_frame,
                offset: self.settings.offset,
            };
            match MtcSender::start(config) {
                Ok(sender) => self.mtc = Some(sender),
                Err(e) => {
                    self.error = Some(e.to_string());
                    self.mtc_enabled = false;
                }
            }
        } else {
            self.mtc = None;
        }
    }

    fn toggle_midi(&mut self) {
        if self.midi_enabled {
            let device = self.settings.midi_device.trim().to_string();
            let config = MidiEventConfig {
                device_name: if device.is_empty() { None } else { Some(device) },
                mapping: MidiEventConfig::default().mapping,
            };
            match MidiEventSender::new(&config) {
                Ok(sender) => self.midi = Some(sender),
                Err(e) => {
                    self.error = Some(e.to_string());
                    self.midi_enabled = false;
                }
            }
        } else {
            self.midi = None;
        }
    }

    fn connect_tcp(&mut self) {
        let config = TcpConfig {
            ip: self.settings.tcp_ip.clone(),
            port: self.settings.tcp_port,
        };
        let mut client = Ma2TcpClient::new(config);
        match client.connect() {
            Ok(()) => self.tcp = Some(client),
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn disconnect_tcp(&mut self) {
        if let Some(mut c) = self.tcp.take() {
            c.disconnect();
        }
    }

    /// Dispara eventos (MIDI + TCP) para marcadores cruzados entre last_pos e pos.
    fn fire_live_events(&mut self, last_pos: f64, pos: f64) {
        if pos <= last_pos {
            return;
        }
        for m in self.manager.markers() {
            if m.time_sec > last_pos && m.time_sec <= pos {
                if let Some(midi) = &mut self.midi {
                    let _ = midi.send(m);
                }
                if let Some(tcp) = &mut self.tcp
                    && tcp.is_connected()
                {
                    let cmd = match m.marker_type {
                        MarkerType::Go | MarkerType::Toggle | MarkerType::Load => {
                            format!("Go Executor {}", m.executor)
                        }
                        MarkerType::Pause => format!("Pause Executor {}", m.executor),
                        MarkerType::Goto => format!(
                            "Goto Cue {} Executor {}",
                            m.cue_number, m.executor
                        ),
                    };
                    let _ = tcp.send_command(&cmd);
                }
            }
        }
    }

    // ------------------------------------------------------------ UI (menus)

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.menu_button("Arquivo", |ui| {
                if ui.button("Novo projeto").clicked() {
                    self.new_project();
                    ui.close();
                }
                if ui.button("Abrir projeto...").clicked() {
                    self.open_project();
                    ui.close();
                }
                ui.separator();
                if ui.button("Salvar").clicked() {
                    self.save_project(false);
                    ui.close();
                }
                if ui.button("Salvar como...").clicked() {
                    self.save_project(true);
                    ui.close();
                }
            });
            ui.menu_button("Exportar", |ui| {
                if ui.button("CSV...").clicked() {
                    self.export_file(ExportKind::Csv);
                    ui.close();
                }
                if ui.button("XML...").clicked() {
                    self.export_file(ExportKind::Xml);
                    ui.close();
                }
                if ui.button("Macro MA2 (.xml)...").clicked() {
                    self.export_file(ExportKind::Ma2Macro);
                    ui.close();
                }
                if ui.button("Arquivo MIDI (.mid)...").clicked() {
                    self.export_file(ExportKind::Midi);
                    ui.close();
                }
            });
            ui.menu_button("Ao vivo", |ui| {
                let mut mtc = self.mtc_enabled;
                if ui.checkbox(&mut mtc, "Enviar MTC").changed() {
                    self.mtc_enabled = mtc;
                    self.toggle_mtc();
                    ui.close();
                }
                let mut midi = self.midi_enabled;
                if ui.checkbox(&mut midi, "Eventos MIDI").changed() {
                    self.midi_enabled = midi;
                    self.toggle_midi();
                    ui.close();
                }
                ui.separator();
                if ui.button("Conectar MA2 (TCP)...").clicked() {
                    self.show_settings = true;
                    ui.close();
                }
            });
            ui.menu_button("Configurações", |ui| {
                if ui.button("Configurações...").clicked() {
                    self.show_settings = true;
                    ui.close();
                }
            });
            if self.decoding {
                ui.separator();
                ui.spinner();
                ui.label("Decodificando...");
            }
            if !self.project_name.is_empty() {
                ui.separator();
                ui.label(
                    egui::RichText::new(format!("Projeto: {}", self.project_name)).weak(),
                );
            }
        });
    }

    // ----------------------------------------------------- UI (painéis)

    fn transport_bar(&mut self, ui: &mut egui::Ui) {
        let pos = self.current_position();
        let tc = self.current_timecode(pos);
        let mut data = TransportData {
            position_sec: pos,
            duration_sec: self.duration_sec,
            is_playing: self.playback.as_ref().is_some_and(|p| p.is_playing()),
            volume: self.volume,
            current_timecode: &tc,
            fps: self.settings.fps,
            drop_frame: self.settings.drop_frame,
            has_audio: self.playback.is_some(),
        };
        let action = transport::show(ui, &mut data);
        if action.play
            && let Some(pb) = &mut self.playback
        {
            pb.play();
        }
        if action.pause
            && let Some(pb) = &mut self.playback
        {
            pb.pause();
        }
        if action.stop
            && let Some(pb) = &mut self.playback
        {
            pb.stop();
        }
        if let Some(sec) = action.seek_to
            && let Some(pb) = &mut self.playback
        {
            pb.seek(sec);
        }
        if let Some(v) = action.volume {
            self.volume = v;
            if let Some(pb) = &mut self.playback {
                pb.set_volume(v);
            }
        }
    }

    fn marker_list(&mut self, ui: &mut egui::Ui) {
        let action = marker_panel::show(ui, &self.manager, self.selected_marker);
        if action.add {
            let pos = self.current_position();
            self.add_marker_at(pos);
        }
        if let Some(id) = action.edit
            && let Some(m) = self.manager.get(id)
        {
            self.editing = Some(MarkerEditDraft::from_marker(m));
        }
        if let Some(id) = action.remove {
            self.manager.remove(id);
            if self.selected_marker == Some(id) {
                self.selected_marker = None;
            }
        }
        if let Some(id) = action.select {
            self.selected_marker = Some(id);
        }
    }

    fn timeline_view(&mut self, ui: &mut egui::Ui) {
        let position_sec = self.current_position();
        let input = TimelineInput {
            waveform: self.waveform.as_ref(),
            sample_rate: self.sample_rate,
            markers: self.manager.markers(),
            position_sec,
            duration_sec: self.duration_sec,
        };
        let resp = timeline::show(ui, &mut self.timeline, &input);
        if let Some(sec) = resp.seek_to
            && let Some(pb) = &mut self.playback
        {
            pb.seek(sec);
        }
        if let Some(sec) = resp.add_marker_at {
            self.add_marker_at(sec);
        }
        if let Some(id) = resp.remove_marker {
            self.manager.remove(id);
            if self.selected_marker == Some(id) {
                self.selected_marker = None;
            }
        }
        if let Some(id) = resp.select_marker {
            self.selected_marker = Some(id);
        }
    }
}

impl eframe::App for ShowtimeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_decode();
        let ctx = ui.ctx().clone();

        egui::Panel::top("menu").show(ui, |ui| self.menu_bar(ui));
        egui::Panel::bottom("transport")
            .exact_size(42.0)
            .show(ui, |ui| self.transport_bar(ui));
        egui::Panel::left("markers")
            .default_size(340.0)
            .resizable(true)
            .show(ui, |ui| self.marker_list(ui));
        egui::CentralPanel::default().show(ui, |ui| self.timeline_view(ui));

        // Janela de configurações.
        if self.show_settings {
            let mut open = true;
            egui::Window::new("Configurações")
                .open(&mut open)
                .show(&ctx, |ui| {
                    let changed = self.settings.show(ui);
                    if changed {
                        self.recompute_marker_timecodes();
                    }
                    ui.separator();
                    ui.heading("Conexão GrandMA2");
                    let connected = self.tcp.as_ref().is_some_and(|c| c.is_connected());
                    ui.label(if connected {
                        "Status: conectado"
                    } else {
                        "Status: desconectado"
                    });
                    if ui
                        .button(if connected { "Desconectar" } else { "Conectar" })
                        .clicked()
                    {
                        if connected {
                            self.disconnect_tcp();
                        } else {
                            self.connect_tcp();
                        }
                    }
                    if self.mtc.is_some() {
                        ui.label("MTC: ativo");
                    }
                    if self.midi.is_some() {
                        ui.label("Eventos MIDI: ativo");
                    }
                });
            self.show_settings = open;
        }

        // Diálogo de edição de marcador.
        let mut edit_open = self.editing.is_some();
        let edit_result = self
            .editing
            .as_mut()
            .and_then(|draft| marker_panel::edit_window(&ctx, &mut edit_open, draft));
        if let Some(res) = edit_result {
            if res == MarkerEditResult::Save {
                if let Some(draft) = self.editing.take() {
                    let id = draft.id;
                    self.manager.update(id, |m| draft.apply_to(m));
                    self.recompute_marker_timecodes();
                }
            } else {
                self.editing = None;
            }
        } else if !edit_open {
            self.editing = None;
        }

        // Mensagem de erro (janela).
        let mut clear_error = false;
        if let Some(err) = &self.error {
            egui::Window::new("Erro").show(&ctx, |ui| {
                ui.colored_label(egui::Color32::from_rgb(230, 80, 80), err);
                if ui.button("OK").clicked() {
                    clear_error = true;
                }
            });
        }
        if clear_error {
            self.error = None;
        }

        // Sync ao vivo + repaint contínuo enquanto toca.
        let playing = self.playback.as_ref().is_some_and(|p| p.is_playing());
        let pos = self.current_position();
        if let Some(mtc) = &self.mtc {
            mtc.set_position_sec(pos);
        }
        if playing {
            self.fire_live_events(self.last_pos, pos);
        }
        self.last_pos = pos;
        if playing || self.decoding {
            ctx.request_repaint();
        }
    }
}

impl Default for ShowtimeApp {
    fn default() -> Self {
        Self::new()
    }
}