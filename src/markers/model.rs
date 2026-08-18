//! Structs de marcadores (verbatim da spec do projeto).

use serde::{Deserialize, Serialize};

use crate::timecode::Timecode;

/// Tipo de disparo da cue no GrandMA2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MarkerType {
    Go,
    Pause,
    Toggle,
    Goto,
    Load,
}

impl MarkerType {
    /// Todos os tipos, na ordem de exibição.
    pub const ALL: [MarkerType; 5] = [
        MarkerType::Go,
        MarkerType::Pause,
        MarkerType::Toggle,
        MarkerType::Goto,
        MarkerType::Load,
    ];

    /// Nome estável (usado em export CSV/XML e rótulos da UI).
    pub fn as_str(&self) -> &'static str {
        match self {
            MarkerType::Go => "go",
            MarkerType::Pause => "pause",
            MarkerType::Toggle => "toggle",
            MarkerType::Goto => "goto",
            MarkerType::Load => "load",
        }
    }
}

/// Marcador de cue ao longo da timeline do áudio.
///
/// `timecode` é um campo calculado a partir de `time_sec`, `frame_rate` e
/// `offset` (preenchido pela UI/gerenciador na edição); é mantido em disco
/// para round-trip exato de projetos.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Marker {
    /// Identificador único dentro do projeto.
    pub id: u32,
    /// Nome legível (ex.: "Intro", "Verse 1").
    pub name: String,
    /// Posição em segundos a partir do início do áudio.
    pub time_sec: f64,
    /// Timecode SMPTE calculado (HH:MM:SS:FF).
    pub timecode: Timecode,
    /// Número da cue no GrandMA2.
    pub cue_number: u32,
    /// Número do executor no console.
    pub executor: u32,
    /// Tipo de disparo.
    pub marker_type: MarkerType,
    /// Cor em "#RRGGBB" (opcional).
    pub color: Option<String>,
    /// Comentário (opcional).
    pub comment: Option<String>,
}

impl Marker {
    /// Cria um marcador com campos padrão.
    pub fn new(
        id: u32,
        name: impl Into<String>,
        time_sec: f64,
        timecode: Timecode,
        cue_number: u32,
        executor: u32,
        marker_type: MarkerType,
    ) -> Self {
        Marker {
            id,
            name: name.into(),
            time_sec,
            timecode,
            cue_number,
            executor,
            marker_type,
            color: None,
            comment: None,
        }
    }
}