//! Envio de MIDI Timecode (MTC): quarter-frame messages (0xF1).
//!
//! MTC envia 8 mensagens quarter-frame por frame SMPTE. Cada mensagem carrega
//! um nibble do timecode atual:
//!
//! ```text
//! qf0: frames low nibble   (0x0F)   qf1: frames high nibble  (0x10)
//! qf2: seconds low nibble  (0x2F)   qf3: seconds high nibble (0x30)
//! qf4: minutes low nibble  (0x4F)   qf5: minutes high nibble (0x50)
//! qf6: hours low nibble    (0x6F)   qf7: hours high + rate   (0x70)
//! ```
//!
//! O nibble do qf7 codifica: bit 0 = horas (bit 4), bits 1-2 = frame rate
//! (0=24, 1=25, 2=29.97DF, 3=30), bit 3 = flag de drop.
//!
//! A thread dedicada recalcula o timecode a partir da posição do áudio a cada
//! frame (1/fps segundos) e envia os 8 quarter-frames do frame atual — a
//! 30fps = 240 msg/s. Prioridade real de thread não é exposta pelo std; o
//! sleep é compensado por Instant para evitar deriva.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use midir::MidiOutput;

use crate::error::ShowtimeError;
use crate::timecode::{conversion, Timecode};

/// Configuração do envio de MTC.
#[derive(Debug, Clone)]
pub struct MtcConfig {
    /// Nome do dispositivo MIDI de saída (substring; None = primeiro porta).
    pub device_name: Option<String>,
    /// Frame rate: 24.0, 25.0, 30.0 ou 29.97.
    pub fps: f64,
    /// Drop-frame (somente 29.97).
    pub drop_frame: bool,
    /// Offset aplicado ao início da música.
    pub offset: Timecode,
}

/// Envia MTC numa thread dedicada sincronizada com a posição do áudio.
pub struct MtcSender {
    handle: Option<JoinHandle<()>>,
    position: Arc<Mutex<f64>>,
    stop: Arc<AtomicBool>,
}

fn select_output_port(
    output: &MidiOutput,
    device_name: &Option<String>,
) -> Result<midir::MidiOutputPort, ShowtimeError> {
    if output.port_count() == 0 {
        return Err(ShowtimeError::Midi("nenhum dispositivo MIDI de saída".into()));
    }
    let port = match device_name {
        Some(name) => output
            .ports()
            .into_iter()
            .find(|p| {
                output
                    .port_name(p)
                    .map(|n| n.contains(name.as_str()))
                    .unwrap_or(false)
            })
            .ok_or_else(|| {
                ShowtimeError::Midi(format!("dispositivo MIDI '{name}' não encontrado"))
            })?,
        None => output.ports().into_iter().next().expect("port_count > 0"),
    };
    Ok(port)
}

/// Codifica os 8 quarter-frames do timecode (bytes de dados, sem status).
fn quarter_frame_nibbles(tc: Timecode, fps: f64, drop_frame: bool) -> [u8; 8] {
    let frames = (tc.frames & 0x0F) as u8;
    let seconds = (tc.seconds & 0x0F) as u8;
    let minutes = (tc.minutes & 0x0F) as u8;
    let hours = (tc.hours & 0x0F) as u8;
    let hour_high = ((tc.hours >> 4) & 0x01) as u8;

    let rate: u8 = if fps == 24.0 {
        0
    } else if fps == 25.0 {
        1
    } else if (fps - 29.97).abs() < 0.001 {
        if drop_frame {
            2
        } else {
            3
        }
    } else {
        3
    };
    let drop = u8::from(drop_frame);

    [
        frames,
        0x10 | ((tc.frames >> 4) & 0x0F) as u8,
        0x20 | seconds,
        0x30 | ((tc.seconds >> 4) & 0x0F) as u8,
        0x40 | minutes,
        0x50 | ((tc.minutes >> 4) & 0x0F) as u8,
        0x60 | hours,
        0x70 | (hour_high | (rate << 1) | (drop << 3)),
    ]
}

impl MtcSender {
    /// Inicia a thread de MTC. Conecta no dispositivo e começa a enviar os
    /// quarter-frames do frame atual.
    pub fn start(config: MtcConfig) -> Result<Self, ShowtimeError> {
        let output = MidiOutput::new("showtime-mtc")
            .map_err(|e| ShowtimeError::Midi(format!("falha ao iniciar MIDI: {e}")))?;
        let port = select_output_port(&output, &config.device_name)?;
        let conn = output
            .connect(&port, "showtime-mtc")
            .map_err(|e| ShowtimeError::Midi(format!("falha ao conectar: {e}")))?;

        let position = Arc::new(Mutex::new(0.0));
        let stop = Arc::new(AtomicBool::new(false));
        let pos2 = Arc::clone(&position);
        let stop2 = Arc::clone(&stop);

        let handle = thread::Builder::new()
            .name("showtime-mtc".into())
            .spawn(move || {
                let mut conn = conn;
                let mut next_frame = Instant::now();
                let mut current: Option<Timecode> = None;
                let frame_dur = Duration::from_secs_f64(1.0 / config.fps);
                while !stop2.load(Ordering::Relaxed) {
                    let pos = *pos2.lock().unwrap();
                    let tc = conversion::seconds_to_timecode(
                        pos,
                        config.fps,
                        config.drop_frame,
                        config.offset,
                    )
                    .unwrap_or(Timecode::ZERO);
                    if current != Some(tc) {
                        for nibble in quarter_frame_nibbles(tc, config.fps, config.drop_frame) {
                            let _ = conn.send(&[0xF1, nibble]);
                        }
                        current = Some(tc);
                    }
                    next_frame += frame_dur;
                    let now = Instant::now();
                    if next_frame > now {
                        thread::sleep(next_frame - now);
                    } else {
                        next_frame = now;
                    }
                }
                conn.close();
            })
            .map_err(|e| ShowtimeError::Midi(format!("falha ao criar thread MTC: {e}")))?;

        Ok(MtcSender {
            handle: Some(handle),
            position,
            stop,
        })
    }

    /// Atualiza a posição do áudio (segundos) que a thread converte em MTC.
    pub fn set_position_sec(&self, sec: f64) {
        *self.position.lock().unwrap() = sec;
    }

    /// Para a thread e desconecta.
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for MtcSender {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quarter_frames_30fps_ndf() {
        let tc = Timecode {
            hours: 1,
            minutes: 2,
            seconds: 3,
            frames: 4,
        };
        let qf = quarter_frame_nibbles(tc, 30.0, false);
        // Frames: 0x04, 0x10. Segundos: 0x20|0x03, 0x30. Minutos: 0x40|0x02, 0x50.
        // Horas: 0x60|0x01, 0x70 | (rate 3 << 1) | drop 0 | hour_high 0 = 0x76.
        assert_eq!(qf, [0x04, 0x10, 0x23, 0x30, 0x42, 0x50, 0x61, 0x76]);
    }

    #[test]
    fn quarter_frames_2997_df_rate_bits() {
        let tc = Timecode {
            hours: 1,
            minutes: 2,
            seconds: 3,
            frames: 4,
        };
        let qf = quarter_frame_nibbles(tc, 29.97, true);
        // rate 2 << 1 = 4, drop << 3 = 8, hour_high 0 -> 0x70 | 12 = 0x7C.
        assert_eq!(qf[7], 0x7C);
    }

    #[test]
    fn quarter_frames_25fps_rate_bits() {
        let tc = Timecode::ZERO;
        let qf = quarter_frame_nibbles(tc, 25.0, false);
        // rate 1 << 1 = 2 -> 0x72.
        assert_eq!(qf[7], 0x72);
    }

    #[test]
    fn quarter_frames_frame_carry() {
        let tc = Timecode {
            hours: 0,
            minutes: 0,
            seconds: 0,
            frames: 0x1F, // 31
        };
        let qf = quarter_frame_nibbles(tc, 30.0, false);
        assert_eq!(qf[0], 0x0F); // low nibble
        assert_eq!(qf[1], 0x11); // high nibble = 1
    }
}