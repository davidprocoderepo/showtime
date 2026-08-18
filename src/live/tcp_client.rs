//! Conexão TCP com GrandMA2 (protocolo NÃO-oficial).
//!
//! Um comando por linha (`Go Executor 1`), terminado com `\n`. O MA2 aceita
//! conexões de rede para comandos de console; documentar que depende do
//! console aceitar (protocolo não documentado oficialmente).
//!
//! Como o tokio aqui NÃO tem a feature `macros` (sem `#[tokio::main]`), o
//! runtime é criado manualmente e as operações são síncronas via
//! `Runtime::block_on`.

use std::net::SocketAddr;

use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::runtime::Runtime;

use crate::error::ShowtimeError;

/// Configuração da conexão TCP com o console.
#[derive(Debug, Clone)]
pub struct TcpConfig {
    pub ip: String,
    pub port: u16,
}

impl Default for TcpConfig {
    fn default() -> Self {
        TcpConfig {
            ip: "192.168.1.10".to_string(),
            port: 3000,
        }
    }
}

/// Cliente TCP síncrono (block_on) sobre tokio.
pub struct Ma2TcpClient {
    config: TcpConfig,
    runtime: Option<Runtime>,
    stream: Option<TcpStream>,
}

impl Ma2TcpClient {
    pub fn new(config: TcpConfig) -> Self {
        Ma2TcpClient {
            config,
            runtime: None,
            stream: None,
        }
    }

    /// Conecta no console (IP:porta). Reconecta se já houver uma conexão.
    pub fn connect(&mut self) -> Result<(), ShowtimeError> {
        self.disconnect();
        let runtime = Runtime::new()
            .map_err(|e| ShowtimeError::Network(format!("falha ao criar runtime tokio: {e}")))?;
        let addr: SocketAddr = format!("{}:{}", self.config.ip, self.config.port)
            .parse()
            .map_err(|e| {
                ShowtimeError::Network(format!(
                    "endereço inválido {}:{}: {e}",
                    self.config.ip, self.config.port
                ))
            })?;
        let stream = runtime
            .block_on(TcpStream::connect(addr))
            .map_err(|e| {
                ShowtimeError::Network(format!(
                    "falha ao conectar em {}:{}: {e}",
                    self.config.ip, self.config.port
                ))
            })?;
        self.runtime = Some(runtime);
        self.stream = Some(stream);
        log::info!("TCP conectado em {}:{}", self.config.ip, self.config.port);
        Ok(())
    }

    /// Envia um comando MA2 (ex.: `Go Executor 1`), terminado com newline.
    pub fn send_command(&mut self, cmd: &str) -> Result<(), ShowtimeError> {
        let stream = self
            .stream
            .as_mut()
            .ok_or_else(|| ShowtimeError::Network("não conectado ao MA2".into()))?;
        let runtime = self
            .runtime
            .as_ref()
            .ok_or_else(|| ShowtimeError::Network("runtime ausente".into()))?;
        let line = format!("{cmd}\n");
        runtime
            .block_on(async {
                stream.write_all(line.as_bytes()).await?;
                stream.flush().await
            })
            .map_err(|e| ShowtimeError::Network(format!("falha ao enviar '{cmd}': {e}")))?;
        Ok(())
    }

    /// Desconecta (fecha o socket).
    pub fn disconnect(&mut self) {
        if let Some(stream) = self.stream.take() {
            let runtime = self.runtime.take();
            if let Some(rt) = runtime {
                rt.block_on(async {
                    let mut s = stream;
                    let _ = s.shutdown().await;
                });
            }
        }
    }

    pub fn is_connected(&self) -> bool {
        self.stream.is_some()
    }
}

impl Drop for Ma2TcpClient {
    fn drop(&mut self) {
        self.disconnect();
    }
}