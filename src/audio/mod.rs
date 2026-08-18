//! Módulo de áudio: decodificação (symphonia), waveform e playback (rodio).
//!
//! CONTRATO (definido no plano; NÃO alterar a interface pública):
//! - `decoder::DecodedAudio { samples: Vec<f32> (interleaved), sample_rate: u32,
//!   channels: u16, duration_sec: f64 }`
//! - `decoder::decode_file(path) -> Result<DecodedAudio, ShowtimeError>`
//! - `waveform::Waveform { peaks: Vec<(f32, f32)> (min,max normalizados),
//!   block_size: usize }`
//! - `waveform::compute_peaks(samples: &[f32], block_size: usize, channels:
//!   u16) -> Waveform` (downmix mono antes de pegar min/max por bloco)
//! - `playback::Playback` (estado play/pause/stop/seek sobre o PCM já
//!   decodificado; rodio `DeviceSinkBuilder` + `Player`)

pub mod decoder;
pub mod playback;
pub mod waveform;