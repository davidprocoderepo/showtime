//! Playback de áudio com rodio 0.22: transporte play/pause/stop/seek.
//!
//! API rodio 0.22 (NÃO usar tutoriais antigos de `OutputStream`/`Sink`):
//! `DeviceSinkBuilder::open_default_sink()` + `Player::connect_new(&sink.mixer())`
//! + `player.append(source)`. O `Player` suporta `try_seek`, mas para simplificar
//!   o seek recria o source a partir da posição (sem clonar as amostras: o source
//!   é um iterador sobre um `Arc<Vec<f32>>` compartilhado).
//!
//! O relógio de posição é mantido pelo próprio [`Playback`] (Instant), então a
//! UI funciona mesmo sem dispositivo de áudio (modo silencioso). Não faz I/O
//! bloqueante.

use std::num::{NonZeroU16, NonZeroU32};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rodio::source::Source;
use rodio::{ChannelCount, DeviceSinkBuilder, Player, SampleRate};

/// Source iterador sobre amostras já decodificadas (sem cópia).
///
/// Compartilha o `Arc<Vec<f32>>` com o app; `offset` é o índice da primeira
/// amostra (seek). Amostras são interleaved.
struct PcmSource {
    samples: Arc<Vec<f32>>,
    channels: u16,
    sample_rate: u32,
    offset: usize,
}

impl PcmSource {
    fn new(samples: Arc<Vec<f32>>, channels: u16, sample_rate: u32, offset: usize) -> Self {
        PcmSource {
            samples,
            channels,
            sample_rate,
            offset,
        }
    }
}

impl Iterator for PcmSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let s = self.samples.get(self.offset).copied();
        if s.is_some() {
            self.offset += 1;
        }
        s
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.samples.len().saturating_sub(self.offset);
        (remaining, Some(remaining))
    }
}

impl Source for PcmSource {
    fn current_span_len(&self) -> Option<usize> {
        Some(self.samples.len().saturating_sub(self.offset))
    }

    fn channels(&self) -> ChannelCount {
        NonZeroU16::new(self.channels).expect("channels > 0")
    }

    fn sample_rate(&self) -> SampleRate {
        NonZeroU32::new(self.sample_rate).expect("sample_rate > 0")
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f64(
            self.samples.len() as f64 / self.channels as f64 / self.sample_rate as f64,
        ))
    }
}

/// Transporte sobre o PCM já decodificado.
///
/// O estado é self-timed: `position_sec()` avança pelo tempo real decorrido
/// quando tocando, mesmo sem dispositivo de áudio (modo silencioso).
pub struct Playback {
    samples: Arc<Vec<f32>>,
    sample_rate: u32,
    channels: u16,
    /// Dispositivo de saída (None = modo silencioso).
    /// Mantido apenas para manter o dispositivo aberto (vida útil).
    #[allow(dead_code)]
    sink: Option<rodio::MixerDeviceSink>,
    player: Option<Player>,
    playing: bool,
    position_sec: f64,
    last_tick: Option<Instant>,
}

impl Playback {
    /// Cria o transporte; tenta abrir o dispositivo de áudio padrão. Se falhar,
    /// opera em modo silencioso (posição avança, sem som).
    pub fn new(samples: Arc<Vec<f32>>, sample_rate: u32, channels: u16) -> Self {
        let mut sink = None;
        let mut player = None;
        match DeviceSinkBuilder::open_default_sink() {
            Ok(s) => {
                let p = Player::connect_new(s.mixer());
                sink = Some(s);
                player = Some(p);
            }
            Err(e) => {
                log::warn!("sem dispositivo de áudio, modo silencioso: {e}");
            }
        }
        Playback {
            samples,
            sample_rate,
            channels,
            sink,
            player,
            playing: false,
            position_sec: 0.0,
            last_tick: None,
        }
    }

    fn source_at(&self, sec: f64) -> Option<PcmSource> {
        let frame = (sec * self.sample_rate as f64).round() as usize;
        let offset = frame.saturating_mul(self.channels as usize);
        if offset >= self.samples.len() {
            return None;
        }
        Some(PcmSource::new(
            Arc::clone(&self.samples),
            self.channels,
            self.sample_rate,
            offset,
        ))
    }

    /// Toca a partir da posição atual.
    pub fn play(&mut self) {
        if self.position_sec >= self.duration_sec() {
            self.position_sec = 0.0;
        }
        if let Some(player) = &self.player
            && let Some(src) = self.source_at(self.position_sec)
        {
            player.clear();
            player.append(src);
            player.play();
        }
        self.playing = true;
        self.last_tick = Some(Instant::now());
    }

    /// Pausa, congelando a posição.
    pub fn pause(&mut self) {
        self.advance_clock();
        if let Some(player) = &self.player {
            player.pause();
        }
        self.playing = false;
        self.last_tick = None;
    }

    /// Para e volta ao início.
    pub fn stop(&mut self) {
        if let Some(player) = &self.player {
            player.stop();
        }
        self.playing = false;
        self.position_sec = 0.0;
        self.last_tick = None;
    }

    /// Move a posição (se tocando, reinicia o source na nova posição).
    pub fn seek(&mut self, sec: f64) {
        let clamped = sec.clamp(0.0, self.duration_sec());
        self.advance_clock();
        self.position_sec = clamped;
        self.last_tick = Some(Instant::now());
        if self.playing
            && let Some(player) = &self.player
            && let Some(src) = self.source_at(clamped)
        {
            player.clear();
            player.append(src);
            player.play();
        }
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// Posição atual em segundos (avança o relógio interno).
    pub fn position_sec(&mut self) -> f64 {
        self.advance_clock();
        self.position_sec
    }

    pub fn duration_sec(&self) -> f64 {
        self.samples.len() as f64 / self.channels as f64 / self.sample_rate as f64
    }

    /// Controla o volume (0.0..=1.0).
    pub fn set_volume(&mut self, volume: f32) {
        if let Some(player) = &self.player {
            player.set_volume(volume.clamp(0.0, 1.0));
        }
    }

    fn advance_clock(&mut self) {
        if self.playing
            && let Some(t) = self.last_tick
        {
            let elapsed = t.elapsed().as_secs_f64();
            self.position_sec += elapsed;
            self.last_tick = Some(Instant::now());
            if self.position_sec >= self.duration_sec() {
                self.position_sec = self.duration_sec();
                self.playing = false;
                if let Some(player) = &self.player {
                    player.stop();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Playback sem dispositivo (clock puro).
    fn silent() -> Playback {
        let samples = Arc::new(vec![0.0f32; 44100 * 2]);
        let mut p = Playback::new(samples, 44100, 2);
        p.sink = None;
        p.player = None;
        p
    }

    #[test]
    fn duration_from_samples() {
        let p = silent();
        assert!((p.duration_sec() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn seek_clamps_to_duration() {
        let mut p = silent();
        p.seek(5.0);
        assert!((p.position_sec - 1.0).abs() < 1e-6);
        p.seek(-2.0);
        assert_eq!(p.position_sec, 0.0);
    }

    #[test]
    fn clock_advances_while_playing() {
        let mut p = silent();
        p.play();
        std::thread::sleep(std::time::Duration::from_millis(50));
        let pos = p.position_sec();
        assert!(pos > 0.0 && pos < 1.0, "posição: {pos}");
    }

    #[test]
    fn pause_freezes_position() {
        let mut p = silent();
        p.play();
        std::thread::sleep(std::time::Duration::from_millis(30));
        p.pause();
        let frozen = p.position_sec();
        std::thread::sleep(std::time::Duration::from_millis(30));
        assert_eq!(p.position_sec(), frozen);
    }

    #[test]
    fn stop_resets_position() {
        let mut p = silent();
        p.play();
        std::thread::sleep(std::time::Duration::from_millis(30));
        p.stop();
        assert_eq!(p.position_sec(), 0.0);
        assert!(!p.is_playing());
    }

    #[test]
    fn source_iterates_remaining_samples() {
        let samples = Arc::new(vec![0.1f32, 0.2, 0.3, 0.4, 0.5, 0.6]);
        let src = PcmSource::new(samples, 2, 3, 2);
        assert_eq!(src.current_span_len(), Some(4));
        assert_eq!(src.size_hint(), (4, Some(4)));
        let collected: Vec<f32> = src.collect();
        assert_eq!(collected, vec![0.3, 0.4, 0.5, 0.6]);
    }

    #[test]
    fn new_does_not_panic_without_device() {
        // Deve funcionar mesmo sem áudio (modo silencioso).
        let _ = Playback::new(Arc::new(Vec::new()), 44100, 2);
    }
}