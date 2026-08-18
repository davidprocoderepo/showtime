//! Widget de timeline: waveform + marcadores + cursor de reprodução.
//!
//! Interações:
//! - clique primário em um marcador → seleciona
//! - clique secundário em um marcador → remove
//! - duplo clique em área vazia → adiciona marcador na posição
//! - arrastar → seek
//! - scroll → zoom; Shift+scroll → rolagem horizontal

use egui::{Align2, Color32, FontId, PointerButton, Pos2, Rect, Sense, Stroke, Vec2};

use crate::audio::waveform::Waveform;
use crate::markers::model::Marker;

/// Ações pedidas pela timeline.
#[derive(Debug, Default)]
pub struct TimelineResponse {
    pub seek_to: Option<f64>,
    pub add_marker_at: Option<f64>,
    pub remove_marker: Option<u32>,
    pub select_marker: Option<u32>,
}

/// Estado de visualização (zoom px/s e rolagem em segundos).
#[derive(Debug, Clone)]
pub struct TimelineState {
    pub zoom: f64,
    pub scroll_sec: f64,
}

impl Default for TimelineState {
    fn default() -> Self {
        TimelineState {
            zoom: 30.0,
            scroll_sec: 0.0,
        }
    }
}

/// Entradas do widget (imutáveis durante o frame).
pub struct TimelineInput<'a> {
    pub waveform: Option<&'a Waveform>,
    pub sample_rate: u32,
    pub markers: &'a [Marker],
    pub position_sec: f64,
    pub duration_sec: f64,
}

/// Converte uma cor "#RRGGBB" em [`Color32`] (None se inválida).
pub(crate) fn parse_hex_color(s: &str) -> Option<Color32> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some(Color32::from_rgb(r, g, b))
}

/// Renderiza a timeline e devolve as ações do usuário.
pub fn show(
    ui: &mut egui::Ui,
    state: &mut TimelineState,
    input: &TimelineInput,
) -> TimelineResponse {
    let mut out = TimelineResponse::default();

    let size = Vec2::new(ui.available_width(), ui.available_height().max(140.0));
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let painter = ui.painter_at(rect);

    // Fundo.
    painter.rect_filled(rect, 4.0, Color32::from_gray(26));

    let duration = input.duration_sec.max(0.001);
    // Zoom mínimo garante que a música inteira caiba na largura.
    let min_zoom = rect.width() as f64 / duration;
    let zoom = state.zoom.max(min_zoom);
    let x_of = |sec: f64| rect.left() + ((sec - state.scroll_sec) * zoom) as f32;

    // Waveform (picos min/max por bloco).
    if let Some(w) = input.waveform {
        let mid_y = rect.center().y;
        let half_h = (rect.height() / 2.0) - 10.0;
        let sec_per_block = w.block_size as f64 / input.sample_rate.max(1) as f64;
        let stride = ((w.peaks.len() as f64) / (rect.width() as f64 / 2.0).max(1.0))
            .ceil()
            .max(1.0) as usize;
        for i in (0..w.peaks.len()).step_by(stride) {
            let (lo, hi) = w.peaks[i];
            let x = x_of(i as f64 * sec_per_block);
            if x < rect.left() - 2.0 || x > rect.right() + 2.0 {
                continue;
            }
            let y0 = mid_y - hi.abs().min(1.0) * half_h;
            let y1 = mid_y - lo.abs().min(1.0) * half_h;
            painter.line_segment(
                [Pos2::new(x, y0), Pos2::new(x, y1)],
                Stroke::new(1.0, Color32::from_gray(130)),
            );
        }
    }

    // Marcadores: linha vertical + rótulo com nome.
    let mut marker_rects: Vec<(u32, Rect)> = Vec::new();
    for m in input.markers {
        let x = x_of(m.time_sec);
        if x < rect.left() - 60.0 || x > rect.right() + 60.0 {
            continue;
        }
        let color = m
            .color
            .as_deref()
            .and_then(parse_hex_color)
            .unwrap_or(Color32::from_rgb(255, 200, 40));
        painter.vline(
            x,
            egui::Rangef::new(rect.top() + 2.0, rect.bottom() - 2.0),
            Stroke::new(1.5, color),
        );
        let label_pos = Pos2::new(x + 4.0, rect.top() + 2.0);
        painter.text(
            label_pos,
            Align2::LEFT_TOP,
            &m.name,
            FontId::proportional(10.0),
            color,
        );
        marker_rects.push((
            m.id,
            Rect::from_min_size(label_pos, Vec2::new(m.name.len() as f32 * 7.0 + 6.0, 14.0)),
        ));
    }

    // Cursor de reprodução.
    let px = x_of(input.position_sec);
    if px >= rect.left() && px <= rect.right() {
        painter.vline(
            px,
            egui::Rangef::new(rect.top(), rect.bottom()),
            Stroke::new(2.0, Color32::from_rgb(240, 60, 60)),
        );
    }

    // Interações.
    if let Some(pointer) = response.interact_pointer_pos() {
        let sec = ((pointer.x - rect.left()) as f64) / zoom + state.scroll_sec;
        if response.double_clicked() {
            out.add_marker_at = Some(sec.clamp(0.0, duration));
        } else if response.clicked_by(PointerButton::Primary) {
            if let Some((id, _)) = marker_rects.iter().find(|(_, r)| r.contains(pointer)) {
                out.select_marker = Some(*id);
            }
        } else if response.clicked_by(PointerButton::Secondary)
            && let Some((id, _)) = marker_rects.iter().find(|(_, r)| r.contains(pointer))
        {
            out.remove_marker = Some(*id);
        }
        if response.dragged() {
            out.seek_to = Some(sec.clamp(0.0, duration));
        }
    }

    // Zoom (scroll) e rolagem (Shift+scroll).
    let scroll_delta = ui.input(|i| i.smooth_scroll_delta);
    if scroll_delta.y != 0.0 {
        let shift = ui.input(|i| i.modifiers.shift);
        if shift {
            state.scroll_sec = (state.scroll_sec + scroll_delta.y as f64 * 0.05 / zoom).max(0.0);
        } else {
            // Zoom ancorado no ponto sob o cursor (ou no centro).
            let anchor_sec = response
                .hover_pos()
                .map(|p| ((p.x - rect.left()) as f64) / zoom + state.scroll_sec)
                .unwrap_or_else(|| state.scroll_sec + rect.width() as f64 * 0.5 / zoom);
            let old_zoom = zoom;
            state.zoom = (state.zoom * (scroll_delta.y as f64 * 0.01).exp()).clamp(2.0, 800.0);
            state.scroll_sec = anchor_sec - (anchor_sec - state.scroll_sec) * old_zoom / state.zoom;
            state.scroll_sec = state.scroll_sec.max(0.0);
        }
    }

    out
}