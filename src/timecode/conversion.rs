//! Conversões entre segundos e [`Timecode`] com frame rate, drop-frame e offset.
//!
//! Toda a aritmética é em segundos `f64` (base: amostras de áudio → segundos).
//!
//! # Drop-frame (29.97)
//!
//! Contagem drop-frame (SMPTE) pula os frames de contagem 00 e 01 a cada
//! minuto, exceto a cada 10º minuto. O objetivo é que o display acompanhe o
//! tempo real: em 60s reais o display lê `00:01:00:00`, e em 1h real lê
//! `01:00:00:00` (107892 frames reais a 29.97fps).
//!
//! Implementação por construção: dado o frame real `n`, o valor nominal
//! exibido é `D(n) = n + 2 * E(n)`, onde `E(n)` é o número de eventos de drop
//! cuja fronteira de minuto (`B_k = round(k * 60 * 29.97)`) ocorre até `n`.
//! A inversa resolve `n = D - 2 * E(n)` por ponto fixo (converge em poucas
//! iterações, pois `E` é monótona e muda de 1 em 1).

use super::Timecode;
use crate::error::ShowtimeError;

/// Frame rate nominal (inteiro) para divisão do display.
fn fps_rounded(fps: f64) -> Result<u32, ShowtimeError> {
    if fps <= 0.0 {
        return Err(ShowtimeError::InvalidTimecode(format!(
            "frame rate inválido: {fps}"
        )));
    }
    Ok(fps.round() as u32)
}

/// Frames reais (inteiros) correspondentes a `seconds` no frame rate dado.
fn real_frames(seconds: f64, fps: f64) -> Result<u64, ShowtimeError> {
    if seconds < 0.0 {
        return Err(ShowtimeError::InvalidTimecode(format!(
            "tempo negativo: {seconds}s"
        )));
    }
    Ok((seconds * fps).round() as u64)
}

/// Fronteira de minuto `k` em frames reais (29.97): `round(k * 60 * 29.97)`.
fn minute_boundary_frames(k: u64) -> u64 {
    (k as f64 * 60.0 * 29.97).round() as u64
}

/// Número de eventos de drop (cada um pula 2 contadores) até o frame real `n`,
/// para drop-frame 29.97. Evento no minuto `k` conta quando `k % 10 != 0` e a
/// fronteira `B_k <= n`.
fn drop_events(n: u64) -> u64 {
    let mut events = 0u64;
    let mut k = 1u64;
    loop {
        let b = minute_boundary_frames(k);
        if b > n {
            break;
        }
        if !k.is_multiple_of(10) {
            events += 1;
        }
        k += 1;
    }
    events
}

/// Valor nominal de display (`D`) a partir do frame real `n`.
fn nominal_frames(n: u64, _fps: f64, drop_frame: bool) -> u64 {
    if drop_frame {
        n + 2 * drop_events(n)
    } else {
        n
    }
}

/// Frame real `n` a partir do valor nominal de display `D` (inversa exata por
/// ponto fixo; para drop-frame, `n = D - 2*E(n)` com `E` monótona).
#[cfg_attr(not(test), allow(dead_code))]
fn real_from_nominal(d: u64, _fps: f64, drop_frame: bool) -> Result<u64, ShowtimeError> {
    if !drop_frame {
        return Ok(d);
    }
    let mut n = d;
    for _ in 0..32 {
        let next = d - 2 * drop_events(n);
        if next == n {
            return Ok(n);
        }
        n = next;
    }
    Err(ShowtimeError::InvalidTimecode(
        "falha ao inverter contagem drop-frame".into(),
    ))
}

/// Converte um [`Timecode`] para frames nominais de display no frame rate dado.
#[cfg_attr(not(test), allow(dead_code))]
fn tc_to_nominal(tc: Timecode, fps_rounded: u32) -> u64 {
    tc.hours as u64 * 3600 * fps_rounded as u64
        + tc.minutes as u64 * 60 * fps_rounded as u64
        + tc.seconds as u64 * fps_rounded as u64
        + tc.frames as u64
}

/// Converte frames nominais de display em [`Timecode`].
fn nominal_to_tc(n: u64, fps_rounded: u32) -> Timecode {
    let fpsr = fps_rounded as u64;
    let hours = n / (3600 * fpsr);
    let rem = n % (3600 * fpsr);
    let minutes = rem / (60 * fpsr);
    let rem = rem % (60 * fpsr);
    let seconds = rem / fpsr;
    let frames = rem % fpsr;
    Timecode {
        hours: hours as u32,
        minutes: minutes as u32,
        seconds: seconds as u32,
        frames: frames as u32,
    }
}

/// Soma dois timecodes no domínio do display (com carries).
fn add_tc(a: Timecode, b: Timecode, fps_rounded: u32) -> Timecode {
    let mut frames = a.frames + b.frames;
    let mut seconds = a.seconds + b.seconds;
    let mut minutes = a.minutes + b.minutes;
    let mut hours = a.hours + b.hours;
    if frames >= fps_rounded {
        frames -= fps_rounded;
        seconds += 1;
    }
    if seconds >= 60 {
        seconds -= 60;
        minutes += 1;
    }
    if minutes >= 60 {
        minutes -= 60;
        hours += 1;
    }
    Timecode {
        hours,
        minutes,
        seconds,
        frames,
    }
}

/// Subtrai `b` de `a` no domínio do display (assume `a >= b`).
#[cfg_attr(not(test), allow(dead_code))]
fn sub_tc(a: Timecode, b: Timecode, fps_rounded: u32) -> Timecode {
    let n = tc_to_nominal(a, fps_rounded) - tc_to_nominal(b, fps_rounded);
    nominal_to_tc(n, fps_rounded)
}

/// Valida campos de um [`Timecode`] para o frame rate dado.
fn validate_tc(tc: Timecode, fps_rounded: u32) -> Result<(), ShowtimeError> {
    if tc.frames >= fps_rounded || tc.seconds >= 60 || tc.minutes >= 60 {
        return Err(ShowtimeError::InvalidTimecode(format!(
            "campos fora da faixa para {fps_rounded}fps: {tc}"
        )));
    }
    Ok(())
}

/// Converte segundos (a partir do início do áudio) em [`Timecode`] SMPTE.
///
/// * `seconds` — tempo em segundos, `>= 0`.
/// * `fps` — frame rate (24.0, 25.0, 30.0, 29.97).
/// * `drop_frame` — se `true`, aplica contagem drop-frame (somente 29.97).
/// * `offset` — timecode aplicado ao instante zero do áudio (soma-se ao
///   resultado; o portador é horas, não um wrap de 24h).
///
/// Exemplo: `seconds_to_timecode(12.5333, 30.0, false, 01:00:00:00)` →
/// `01:00:12:16`.
pub fn seconds_to_timecode(
    seconds: f64,
    fps: f64,
    drop_frame: bool,
    offset: Timecode,
) -> Result<Timecode, ShowtimeError> {
    let fpsr = fps_rounded(fps)?;
    if drop_frame && (fps - 29.97).abs() > f64::EPSILON * 100.0 {
        return Err(ShowtimeError::InvalidTimecode(format!(
            "drop-frame só é suportado em 29.97fps (recebido {fps})"
        )));
    }
    validate_tc(offset, fpsr)?;
    let n = real_frames(seconds, fps)?;
    let d = nominal_frames(n, fps, drop_frame);
    Ok(add_tc(nominal_to_tc(d, fpsr), offset, fpsr))
}

/// Converte um [`Timecode`] em segundos reais desde o início do áudio.
///
/// É o inverso de [`seconds_to_timecode`]: o `offset` é subtraído.
#[cfg_attr(not(test), allow(dead_code))]
pub fn timecode_to_seconds(
    tc: Timecode,
    fps: f64,
    drop_frame: bool,
    offset: Timecode,
) -> Result<f64, ShowtimeError> {
    let fpsr = fps_rounded(fps)?;
    if drop_frame && (fps - 29.97).abs() > f64::EPSILON * 100.0 {
        return Err(ShowtimeError::InvalidTimecode(format!(
            "drop-frame só é suportado em 29.97fps (recebido {fps})"
        )));
    }
    validate_tc(tc, fpsr)?;
    validate_tc(offset, fpsr)?;
    let base = sub_tc(tc, offset, fpsr);
    let d = tc_to_nominal(base, fpsr);
    let n = real_from_nominal(d, fps, drop_frame)?;
    Ok(n as f64 / fps)
}

#[cfg(test)]
mod tests {
    //! Os vetores abaixo devem passar quando a implementação estiver pronta.
    //! Eles são os vetores canônicos da indústria (SMPTE drop-frame).

    use super::*;

    #[test]
    fn ndf_30_roundtrip() {
        // 12.5s a 30fps = 375 frames = 00:00:12:15
        let tc = seconds_to_timecode(12.5, 30.0, false, Timecode::ZERO).unwrap();
        assert_eq!(tc, Timecode { hours: 0, minutes: 0, seconds: 12, frames: 15 });
        let back = timecode_to_seconds(tc, 30.0, false, Timecode::ZERO).unwrap();
        assert!((back - 12.5).abs() < 0.05);
    }

    #[test]
    fn offset_applies_after_conversion() {
        let offset: Timecode = "01:00:00:00".parse().unwrap();
        let tc = seconds_to_timecode(12.5333, 30.0, false, offset).unwrap();
        assert_eq!(tc, Timecode { hours: 1, minutes: 0, seconds: 12, frames: 16 });
    }

    #[test]
    fn df_29_97_vectors() {
        // Vetor canônico: 01:00:00:00 (drop-frame) == 107892 frames reais.
        // Em segundos: 3600.0 * 29.97 == 107892.0
        let tc = seconds_to_timecode(3600.0, 29.97, true, Timecode::ZERO).unwrap();
        assert_eq!(tc, Timecode { hours: 1, minutes: 0, seconds: 0, frames: 0 });
        let back = timecode_to_seconds(tc, 29.97, true, Timecode::ZERO).unwrap();
        assert!((back - 3600.0).abs() < 0.05);
    }

    #[test]
    fn df_round_trip_property() {
        // Propriedade: para um conjunto grande de segundos, converter e voltar
        // deve ser consistente (dentro de ~1 frame).
        for s in (0..=7200).map(|x| x as f64 * 1.0) {
            let tc = seconds_to_timecode(s, 29.97, true, Timecode::ZERO).unwrap();
            let back = timecode_to_seconds(tc, 29.97, true, Timecode::ZERO).unwrap();
            assert!(
                (back - s).abs() < 0.05,
                "round-trip falhou em {s}s -> {tc} -> {back}"
            );
        }
    }

    #[test]
    fn invalid_fps_rejected() {
        assert!(seconds_to_timecode(1.0, 0.0, false, Timecode::ZERO).is_err());
    }

    #[test]
    fn ndf_25_and_24() {
        // 1s a 25fps = 25 frames = exatamente 00:00:01:00.
        let tc25 = seconds_to_timecode(1.0, 25.0, false, Timecode::ZERO).unwrap();
        assert_eq!(tc25, Timecode { hours: 0, minutes: 0, seconds: 1, frames: 0 });
        // 0.5s a 25fps = 12.5 frames -> round = 13 -> 00:00:00:13.
        let tc25b = seconds_to_timecode(0.5, 25.0, false, Timecode::ZERO).unwrap();
        assert_eq!(tc25b.frames, 13);
        // 0.5s a 24fps = 12 frames -> 00:00:00:12.
        let tc24 = seconds_to_timecode(0.5, 24.0, false, Timecode::ZERO).unwrap();
        assert_eq!(tc24.frames, 12);
    }

    #[test]
    fn df_minute_boundary() {
        // 60s reais a 29.97 drop-frame -> 00:01:00:00 (1798 frames reais).
        let tc = seconds_to_timecode(60.0, 29.97, true, Timecode::ZERO).unwrap();
        assert_eq!(tc, Timecode { hours: 0, minutes: 1, seconds: 0, frames: 0 });
        // 60.0666s -> 00:01:00:02
        let tc = seconds_to_timecode(60.0666, 29.97, true, Timecode::ZERO).unwrap();
        assert_eq!(tc, Timecode { hours: 0, minutes: 1, seconds: 0, frames: 2 });
    }

    #[test]
    fn df_rejects_non_2997() {
        assert!(seconds_to_timecode(1.0, 30.0, true, Timecode::ZERO).is_err());
    }

    #[test]
    fn tc_with_offset_roundtrip() {
        let offset: Timecode = "01:00:00:00".parse().unwrap();
        for s in (0..=3600).map(|x| x as f64 * 2.0) {
            let tc = seconds_to_timecode(s, 29.97, true, offset).unwrap();
            let back = timecode_to_seconds(tc, 29.97, true, offset).unwrap();
            assert!((back - s).abs() < 0.05, "offset round-trip falhou em {s}s");
        }
    }

    #[test]
    fn invalid_tc_fields_rejected() {
        let bad = Timecode { hours: 0, minutes: 0, seconds: 12, frames: 30 };
        assert!(timecode_to_seconds(bad, 30.0, false, Timecode::ZERO).is_err());
    }
}