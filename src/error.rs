//! Erros de domínio do Showtime.
//!
//! Regra: `thiserror` para erros específicos de domínio; `anyhow` para erros
//! gerais em camadas de orquestração (ex.: UI). Os módulos core usam
//! [`ShowtimeError`].

use thiserror::Error;

/// Erros de domínio do aplicativo.
#[derive(Debug, Error)]
pub enum ShowtimeError {
    /// Falha ao decodificar um arquivo de áudio.
    #[error("erro de decodificação de áudio: {0}")]
    AudioDecode(String),

    /// Arquivo não encontrado ou inacessível.
    #[error("arquivo não encontrado: {0}")]
    FileNotFound(String),

    /// Erro de I/O.
    #[error("erro de I/O: {0}")]
    Io(#[from] std::io::Error),

    /// Erro de serialização JSON.
    #[error("erro de serialização JSON: {0}")]
    SerdeJson(#[from] serde_json::Error),

    /// Erro de serialização YAML.
    #[error("erro de serialização YAML: {0}")]
    SerdeYaml(#[from] yaml_serde::Error),

    /// Timecode inválido (formato HH:MM:SS:FF ou valores fora da faixa).
    #[error("timecode inválido: {0}")]
    InvalidTimecode(String),

    /// Falha relacionada a MIDI (dispositivo, envio).
    #[error("erro MIDI: {0}")]
    Midi(String),

    /// Falha de rede (TCP para GrandMA2).
    #[error("erro de rede: {0}")]
    Network(String),
}