//! Eventos MIDI por marcador: Note On/Off (e MSC reservado para futuro).
//!
//! Cada marcador dispara um Note On (velocity 100) seguido de Note Off no
//! canal 1, na nota mapeada pelo tipo do marcador (default:
//! go=60, pause=61, toggle=62, goto=63, load=64).

use std::collections::HashMap;

use midir::{MidiOutput, MidiOutputConnection};

use crate::error::ShowtimeError;
use crate::markers::model::{Marker, MarkerType};

/// Configuração do envio de eventos MIDI.
#[derive(Debug, Clone)]
pub struct MidiEventConfig {
    /// Nome do dispositivo MIDI de saída (substring; None = primeiro porta).
    pub device_name: Option<String>,
    /// Mapeamento tipo -> nota MIDI.
    pub mapping: HashMap<MarkerType, u8>,
}

impl Default for MidiEventConfig {
    fn default() -> Self {
        let mut mapping = HashMap::new();
        mapping.insert(MarkerType::Go, 60);
        mapping.insert(MarkerType::Pause, 61);
        mapping.insert(MarkerType::Toggle, 62);
        mapping.insert(MarkerType::Goto, 63);
        mapping.insert(MarkerType::Load, 64);
        MidiEventConfig {
            device_name: None,
            mapping,
        }
    }
}

/// Envia Note On/Off no instante do marcador.
pub struct MidiEventSender {
    conn: Option<MidiOutputConnection>,
    mapping: HashMap<MarkerType, u8>,
}

/// Nota MIDI para o tipo do marcador (fallback 60).
fn note_for(mapping: &HashMap<MarkerType, u8>, marker_type: MarkerType) -> u8 {
    mapping.get(&marker_type).copied().unwrap_or(60)
}

impl MidiEventSender {
    /// Conecta no dispositivo de saída selecionado.
    pub fn new(config: &MidiEventConfig) -> Result<Self, ShowtimeError> {
        let output = MidiOutput::new("showtime-midi")
            .map_err(|e| ShowtimeError::Midi(format!("falha ao iniciar MIDI: {e}")))?;
        let ports = output.ports();
        let port = match &config.device_name {
            Some(name) => ports
                .iter()
                .find(|p| {
                    output
                        .port_name(p)
                        .map(|n| n.contains(name.as_str()))
                        .unwrap_or(false)
                })
                .ok_or_else(|| {
                    ShowtimeError::Midi(format!("dispositivo MIDI '{name}' não encontrado"))
                })?,
            None => {
                if ports.is_empty() {
                    return Err(ShowtimeError::Midi(
                        "nenhum dispositivo MIDI de saída".into(),
                    ));
                }
                ports.first().expect("port_count > 0")
            }
        };
        let conn = output
            .connect(port, "showtime-midi")
            .map_err(|e| ShowtimeError::Midi(format!("falha ao conectar: {e}")))?;
        Ok(MidiEventSender {
            conn: Some(conn),
            mapping: config.mapping.clone(),
        })
    }

    /// Envia Note On + Note Off (canal 1) para o marcador.
    pub fn send(&mut self, marker: &Marker) -> Result<(), ShowtimeError> {
        let note = note_for(&self.mapping, marker.marker_type);
        let Some(conn) = self.conn.as_mut() else {
            return Err(ShowtimeError::Midi("conexão MIDI fechada".into()));
        };
        conn.send(&[0x90, note, 100])
            .map_err(|e| ShowtimeError::Midi(format!("falha ao enviar Note On: {e}")))?;
        conn.send(&[0x80, note, 0])
            .map_err(|e| ShowtimeError::Midi(format!("falha ao enviar Note Off: {e}")))?;
        Ok(())
    }
}

impl Drop for MidiEventSender {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take() {
            let _ = conn.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_mapping_covers_all_types() {
        let cfg = MidiEventConfig::default();
        for t in MarkerType::ALL {
            assert!(cfg.mapping.contains_key(&t), "tipo {t:?} sem nota");
        }
        assert_eq!(cfg.mapping.get(&MarkerType::Go), Some(&60));
        assert_eq!(cfg.mapping.get(&MarkerType::Load), Some(&64));
    }

    #[test]
    fn mapping_missing_falls_back_to_60() {
        let empty = HashMap::new();
        assert_eq!(note_for(&empty, MarkerType::Go), 60);

        let mut mapping = HashMap::new();
        mapping.insert(MarkerType::Pause, 72);
        assert_eq!(note_for(&mapping, MarkerType::Pause), 72);
        assert_eq!(note_for(&mapping, MarkerType::Go), 60);
    }
}