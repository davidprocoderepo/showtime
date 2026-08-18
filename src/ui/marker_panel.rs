//! Painel lateral de marcadores: tabela + diálogo de edição.
//!
//! A tabela mostra Nome / Timecode / Cue / Executor / Tipo; o diálogo de
//! edição tem Nome, Tipo (dropdown), Cue, Executor, Cor e Comentário.
//! Enter salva, Esc cancela.

use egui::{Color32, Key};

use crate::markers::{Marker, MarkerManager, MarkerType};

use super::timeline::parse_hex_color;

/// Ações do painel de marcadores.
#[derive(Debug, Default)]
pub struct MarkerPanelAction {
    pub add: bool,
    pub edit: Option<u32>,
    pub remove: Option<u32>,
    pub select: Option<u32>,
}

/// Rascunho do marcador em edição (preenchido no diálogo).
#[derive(Debug, Clone)]
pub struct MarkerEditDraft {
    pub id: u32,
    pub name: String,
    pub marker_type: MarkerType,
    pub cue_number: u32,
    pub executor: u32,
    pub color: String,
    pub comment: String,
}

impl MarkerEditDraft {
    /// Preenche o rascunho a partir de um marcador existente.
    pub fn from_marker(m: &Marker) -> Self {
        MarkerEditDraft {
            id: m.id,
            name: m.name.clone(),
            marker_type: m.marker_type,
            cue_number: m.cue_number,
            executor: m.executor,
            color: m.color.clone().unwrap_or_default(),
            comment: m.comment.clone().unwrap_or_default(),
        }
    }

    /// Aplica o rascunho ao marcador (preservando campos não editados).
    pub fn apply_to(&self, m: &mut Marker) {
        m.name = self.name.trim().to_string();
        m.marker_type = self.marker_type;
        m.cue_number = self.cue_number;
        m.executor = self.executor;
        m.color = Some(self.color.trim().to_string());
        m.comment = Some(self.comment.clone());
    }
}

/// Resultado do diálogo de edição.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerEditResult {
    Save,
    Cancel,
}

/// Renderiza a tabela de marcadores.
pub fn show(
    ui: &mut egui::Ui,
    manager: &MarkerManager,
    selected: Option<u32>,
) -> MarkerPanelAction {
    let mut action = MarkerPanelAction::default();

    ui.horizontal(|ui| {
        ui.heading(format!("Marcadores ({})", manager.markers().len()));
        if ui.button("+ Adicionar").clicked() {
            action.add = true;
        }
    });
    ui.separator();

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for m in manager.markers() {
                let is_selected = selected == Some(m.id);
                let row_color = m
                    .color
                    .as_deref()
                    .and_then(parse_hex_color)
                    .unwrap_or(Color32::from_rgb(255, 200, 40));
                ui.horizontal(|ui| {
                    ui.add_space(2.0);
                    ui.label(egui::RichText::new("■").color(row_color));
                    let label = format!(
                        "{} — {} — Cue {} (Ex {}) [{}]",
                        m.name,
                        m.timecode,
                        m.cue_number,
                        m.executor,
                        m.marker_type.as_str()
                    );
                    let resp = ui.selectable_label(is_selected, label);
                    if resp.clicked() {
                        action.select = Some(m.id);
                    }
                    if ui.small_button("✎").clicked() {
                        action.edit = Some(m.id);
                    }
                    if ui.small_button("🗑").clicked() {
                        action.remove = Some(m.id);
                    }
                });
            }
        });

    action
}

/// Diálogo de edição (janela flutuante). Devolve `Some(result)` quando o
/// usuário confirma ou cancela.
pub fn edit_window(
    ctx: &egui::Context,
    open: &mut bool,
    draft: &mut MarkerEditDraft,
) -> Option<MarkerEditResult> {
    let mut result = None;
    let mut open_local = *open;

    egui::Window::new(format!("Editar marcador #{}", draft.id))
        .open(&mut open_local)
        .show(ctx, |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut draft.name).hint_text("Nome do marcador"),
            );

            egui::ComboBox::from_label("Tipo")
                .selected_text(draft.marker_type.as_str())
                .show_ui(ui, |ui| {
                    for t in MarkerType::ALL {
                        if ui
                            .selectable_label(draft.marker_type == t, t.as_str())
                            .clicked()
                        {
                            draft.marker_type = t;
                        }
                    }
                });

            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(&mut draft.cue_number)
                        .range(1..=9999)
                        .prefix("Cue: "),
                );
                ui.add(
                    egui::DragValue::new(&mut draft.executor)
                        .range(1..=99)
                        .prefix("Executor: "),
                );
            });

            ui.add(
                egui::TextEdit::singleline(&mut draft.color).hint_text("Cor #RRGGBB"),
            );
            ui.add(
                egui::TextEdit::multiline(&mut draft.comment)
                    .desired_rows(2)
                    .hint_text("Comentário"),
            );

            ui.horizontal(|ui| {
                if ui.button("Salvar (Enter)").clicked() {
                    result = Some(MarkerEditResult::Save);
                }
                if ui.button("Cancelar (Esc)").clicked() {
                    result = Some(MarkerEditResult::Cancel);
                }
            });

            if ui.input(|i| i.key_pressed(Key::Enter)) {
                result = Some(MarkerEditResult::Save);
            }
            if ui.input(|i| i.key_pressed(Key::Escape)) {
                result = Some(MarkerEditResult::Cancel);
            }
        });

    if result.is_some() {
        *open = false;
    }
    // Se o usuário fechou a janela pelo X, não salva.
    if !open_local && result.is_none() {
        result = Some(MarkerEditResult::Cancel);
        *open = false;
    }
    result
}