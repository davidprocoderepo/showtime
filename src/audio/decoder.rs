//! Decodificação de arquivos de áudio para PCM f32 (symphonia 0.6).
//!
//! Decodifica UMA vez para `Vec<f32>` interleaved (L,R,L,R,...) e reutiliza
//! para waveform e playback. `decode_file` faz I/O na thread chamadora — a UI
//! deve chamá-lo numa thread de fundo.

use std::fs::File;
use std::path::Path;

use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

use crate::error::ShowtimeError;

/// PCM f32 interleaved decodificado de um arquivo.
#[derive(Debug, Clone)]
pub struct DecodedAudio {
    /// Amostras interleaved (L,R,L,R,...) em [-1.0, 1.0].
    pub samples: Vec<f32>,
    /// Taxa de amostragem (Hz).
    pub sample_rate: u32,
    /// Número de canais.
    pub channels: u16,
    /// Duração total em segundos.
    pub duration_sec: f64,
}

/// Decodifica um arquivo de áudio (MP3/FLAC/WAV/AIFF/OGG/PCM) para PCM f32.
pub fn decode_file(path: &Path) -> Result<DecodedAudio, ShowtimeError> {
    let file = File::open(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ShowtimeError::FileNotFound(path.display().to_string())
        } else {
            ShowtimeError::AudioDecode(format!(
                "falha ao abrir {}: {e}",
                path.display()
            ))
        }
    })?;

    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let fmt_opts = FormatOptions::default();
    let meta_opts = MetadataOptions::default();

    let mut format = symphonia::default::get_probe()
        .probe(&hint, mss, fmt_opts, meta_opts)
        .map_err(|e| ShowtimeError::AudioDecode(format!("formato não suportado: {e}")))?;

    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| ShowtimeError::AudioDecode("nenhuma trilha de áudio encontrada".into()))?;
    let codec_params = track
        .codec_params
        .as_ref()
        .ok_or_else(|| ShowtimeError::AudioDecode("parâmetros do codec ausentes".into()))?;

    let audio_params = codec_params
        .audio()
        .ok_or_else(|| ShowtimeError::AudioDecode("codec de áudio ausente".into()))?;
    let sample_rate = audio_params.sample_rate.unwrap_or(44100);
    let channels = audio_params
        .channels
        .as_ref()
        .map(|c| c.count() as u16)
        .unwrap_or(2);

    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(audio_params, &AudioDecoderOptions::default())
        .map_err(|e| ShowtimeError::AudioDecode(format!("codec não suportado: {e}")))?;

    let track_id = track.id;
    let mut samples: Vec<f32> = Vec::new();

    // Lê e decodifica todos os pacotes da trilha selecionada.
    while let Some(packet) = format
        .next_packet()
        .map_err(|e| ShowtimeError::AudioDecode(format!("erro lendo pacotes: {e}")))?
    {
        if packet.track_id != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(buf) => {
                let n = buf.samples_interleaved();
                let start = samples.len();
                samples.resize(start + n, 0.0);
                buf.copy_to_slice_interleaved(&mut samples[start..]);
            }
            // Erros de decodificação pontuais são pulados; outros encerram.
            Err(Error::DecodeError(_)) => continue,
            Err(_) => break,
        }
    }

    if samples.is_empty() {
        return Err(ShowtimeError::AudioDecode(
            "nenhuma amostra decodificada".into(),
        ));
    }

    let duration_sec = samples.len() as f64 / channels as f64 / sample_rate as f64;

    Ok(DecodedAudio {
        samples,
        sample_rate,
        channels,
        duration_sec,
    })
}