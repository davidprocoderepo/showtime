//! Struct de projeto serializável (verbatim da spec).

use serde::{Deserialize, Serialize};

use crate::markers::model::Marker;
use crate::timecode::Timecode;

/// Projeto completo do Showtime (serializável para JSON ou YAML).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    /// Nome do projeto.
    pub name: String,
    /// Caminho do arquivo de áudio (relativo ou absoluto).
    pub audio_file_path: Option<String>,
    /// Frame rate de timecode: 24.0, 25.0, 30.0, 29.97.
    pub frame_rate: f64,
    /// Se `true`, usa contagem drop-frame (somente 29.97).
    pub drop_frame: bool,
    /// Timecode offset aplicado ao início da música.
    pub timecode_offset: Timecode,
    /// Marcadores do projeto.
    pub markers: Vec<Marker>,
}

impl Default for Project {
    fn default() -> Self {
        Project {
            name: "Novo projeto".to_string(),
            audio_file_path: None,
            frame_rate: 30.0,
            drop_frame: false,
            timecode_offset: Timecode::ZERO,
            markers: Vec::new(),
        }
    }
}