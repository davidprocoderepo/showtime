//! Estrutura [`Timecode`] no formato HH:MM:SS:FF.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Timecode SMPTE: horas, minutos, segundos e frames.
///
/// O campo `frames` está na faixa `0..frames_per_second` do frame rate em uso
/// (ex.: 0..=29 para 30fps); a validação da faixa é responsabilidade das
/// conversões e da UI, não desta struct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timecode {
    pub hours: u32,
    pub minutes: u32,
    pub seconds: u32,
    pub frames: u32,
}

impl Timecode {
    /// Timecode zero (00:00:00:00).
    pub const ZERO: Timecode = Timecode {
        hours: 0,
        minutes: 0,
        seconds: 0,
        frames: 0,
    };
}

impl fmt::Display for Timecode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02}:{:02}:{:02}:{:02}",
            self.hours, self.minutes, self.seconds, self.frames
        )
    }
}

/// Erro ao parsear uma string como [`Timecode`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseTimecodeError(pub String);

impl fmt::Display for ParseTimecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "timecode inválido '{}' (esperado HH:MM:SS:FF)",
            self.0
        )
    }
}

impl std::error::Error for ParseTimecodeError {}

impl FromStr for Timecode {
    type Err = ParseTimecodeError;

    /// Parse de `HH:MM:SS:FF` (ex.: `01:00:12:16`).
    ///
    /// Aceita 1 ou 2 dígitos por campo. Rejeita strings com mais de 4 campos,
    /// campos não numéricos ou valores que não caibam em `u32`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.trim().split(':').collect();
        if parts.len() != 4 {
            return Err(ParseTimecodeError(s.to_string()));
        }
        let parse = |p: &str| -> Result<u32, ParseTimecodeError> {
            p.trim()
                .parse::<u32>()
                .map_err(|_| ParseTimecodeError(s.to_string()))
        };
        Ok(Timecode {
            hours: parse(parts[0])?,
            minutes: parse(parts[1])?,
            seconds: parse(parts[2])?,
            frames: parse(parts[3])?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_formats_zero_padded() {
        let tc = Timecode {
            hours: 1,
            minutes: 0,
            seconds: 12,
            frames: 16,
        };
        assert_eq!(tc.to_string(), "01:00:12:16");
    }

    #[test]
    fn parse_roundtrip() {
        let tc: Timecode = "01:00:12:16".parse().unwrap();
        assert_eq!(tc.to_string(), "01:00:12:16");
        assert_eq!(tc.hours, 1);
        assert_eq!(tc.frames, 16);
    }

    #[test]
    fn parse_single_digits() {
        let tc: Timecode = "0:0:0:0".parse().unwrap();
        assert_eq!(tc, Timecode::ZERO);
    }

    #[test]
    fn parse_rejects_wrong_field_count() {
        assert!("01:00:12".parse::<Timecode>().is_err());
        assert!("01:00:12:16:00".parse::<Timecode>().is_err());
    }

    #[test]
    fn parse_rejects_non_numeric() {
        assert!("aa:00:12:16".parse::<Timecode>().is_err());
    }
}