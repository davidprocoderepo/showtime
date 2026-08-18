//! Arquivo MIDI (.mid) com eventos de marcadores (midly).
//!
//! Formato 0 (uma trilha), tempo fixo de 120 BPM, 480 ticks/beat. Cada
//! marcador vira um Note On/Off no canal 1, posicionado pelo tempo do marcador.
//! A nota é derivada do índice do marcador (36 + idx % 48) para ficar numa
//! faixa audível e estável.

use midly::{Format, Header, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};

use crate::error::ShowtimeError;
use crate::markers::model::Marker;

const TICKS_PER_BEAT: u16 = 480;
const BPM: u32 = 120;

/// Serializa os marcadores como um arquivo MIDI (bytes prontos para `.mid`).
pub fn to_midi_bytes(markers: &[Marker]) -> Result<Vec<u8>, ShowtimeError> {
    // Ticks por segundo no tempo fixo: 480 * 120 / 60 = 960.
    let ticks_per_sec = TICKS_PER_BEAT as f64 * BPM as f64 / 60.0;
    let tempo = 60_000_000u32 / BPM; // microssegundos por semínima (120 BPM)

    let mut track: midly::Track = Vec::new();
    // Tempo no início (delta 0).
    track.push(TrackEvent {
        delta: 0u32.into(),
        kind: TrackEventKind::Meta(midly::MetaMessage::Tempo(tempo.into())),
    });

    let mut prev_tick = 0u32;
    for (idx, m) in markers.iter().enumerate() {
        let tick = (m.time_sec * ticks_per_sec).round() as u32;
        let delta = tick.saturating_sub(prev_tick);
        prev_tick = tick;

        let key: u8 = 36 + (idx % 48) as u8;
        let channel = 0u8;
        track.push(TrackEvent {
            delta: delta.into(),
            kind: TrackEventKind::Midi {
                channel: channel.into(),
                message: MidiMessage::NoteOn { key: key.into(), vel: 100u8.into() },
            },
        });
        track.push(TrackEvent {
            delta: 1u32.into(),
            kind: TrackEventKind::Midi {
                channel: channel.into(),
                message: MidiMessage::NoteOff { key: key.into(), vel: 0u8.into() },
            },
        });
    }

    let smf = Smf {
        header: Header::new(Format::SingleTrack, Timing::Metrical(TICKS_PER_BEAT.into())),
        tracks: vec![track],
    };
    let mut bytes = Vec::new();
    smf.write_std(&mut bytes).map_err(ShowtimeError::Io)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markers::model::MarkerType;
    use crate::timecode::Timecode;

    fn marker(id: u32, time_sec: f64) -> Marker {
        Marker::new(id, "Intro", time_sec, Timecode::ZERO, id, 1, MarkerType::Go)
    }

    #[test]
    fn roundtrip_has_note_pairs() {
        let markers = vec![marker(1, 0.0), marker(2, 1.0), marker(3, 2.5)];
        let bytes = to_midi_bytes(&markers).unwrap();
        assert!(!bytes.is_empty());
        // MThd + 3 blocos: header, track, EOT é interno à track.
        assert!(bytes.starts_with(b"MThd"));

        let parsed = Smf::parse(&bytes).unwrap();
        assert_eq!(parsed.header.format, Format::SingleTrack);
        assert_eq!(parsed.tracks.len(), 1);

        let note_events: usize = parsed.tracks[0]
            .iter()
            .filter(|e| matches!(e.kind, TrackEventKind::Midi { .. }))
            .count();
        // 3 marcadores × (NoteOn + NoteOff) = 6.
        assert_eq!(note_events, 6);
    }

    #[test]
    fn ticks_follow_time() {
        let markers = vec![marker(1, 0.0), marker(2, 1.0)];
        let bytes = to_midi_bytes(&markers).unwrap();
        let parsed = Smf::parse(&bytes).unwrap();
        // 1s = 960 ticks; o segundo NoteOn deve vir 960 ticks depois do primeiro.
        let deltas: Vec<u32> = parsed.tracks[0]
            .iter()
            .map(|e| u32::from(e.delta))
            .collect();
        assert!(deltas.contains(&960), "deltas: {deltas:?}");
    }
}