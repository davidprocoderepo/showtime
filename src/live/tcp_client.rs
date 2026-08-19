//! Conexão TCP com GrandMA2 (protocolo NÃO-oficial).
//!
//! Um comando por linha (`Go Executor 1`), terminado com `\n`. O MA2 aceita
//! conexões de rede para comandos de console; documentar que depende do
//! console aceitar (protocolo não documentado oficialmente).
//!
//! Como o tokio aqui NÃO tem a feature `macros` (sem `#[tokio::main]`), o
//! runtime é criado manualmente e as operações são síncronas via
//! `Runtime::block_on`.
//!
//! Timeouts: `connect` e `send_command` usam `tokio::time::timeout` para
//! falhar rápido (o TCP nativo pode bloquear ~75s em IPs inalcançáveis —
//! travaria a thread da UI). `is_connected()` reflete o socket real: um envio
//! que falha marca o cliente como desconectado.

use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::runtime::Runtime;
use tokio::time::timeout;

use crate::error::ShowtimeError;

/// Tempo máximo para estabelecer a conexão TCP (handshake).
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
/// Tempo máximo para enviar um comando (write + flush).
const SEND_TIMEOUT: Duration = Duration::from_secs(2);

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

    /// Conecta no console (IP:porta) com timeout. Reconecta se já houver uma
    /// conexão.
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
            .block_on(async {
                timeout(CONNECT_TIMEOUT, TcpStream::connect(addr))
                    .await
                    .map_err(|_| {
                        ShowtimeError::Network(format!(
                            "timeout ao conectar em {}:{} ({}s)",
                            self.config.ip,
                            self.config.port,
                            CONNECT_TIMEOUT.as_secs()
                        ))
                    })
                    .and_then(|r| r.map_err(ShowtimeError::Io))
            })?;
        self.runtime = Some(runtime);
        self.stream = Some(stream);
        log::info!("TCP conectado em {}:{}", self.config.ip, self.config.port);
        Ok(())
    }

    /// Envia um comando MA2 (ex.: `Go Executor 1`), terminado com newline,
    /// com timeout. Se o envio falhar (socket morto), marca o cliente como
    /// desconectado para a UI refletir o estado real.
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
        let res = runtime.block_on(async {
            timeout(SEND_TIMEOUT, async {
                stream.write_all(line.as_bytes()).await?;
                stream.flush().await
            })
            .await
            .map_err(|_| {
                ShowtimeError::Network(format!("timeout ao enviar '{cmd}' ({}s)", SEND_TIMEOUT.as_secs()))
            })?
            .map_err(ShowtimeError::Io)
        });
        if let Err(e) = res {
            // Socket morto (console desligado/cabo fora): refletir no estado.
            log::warn!("envio falhou, marcando desconectado: {e}");
            self.stream = None;
            self.runtime = None;
            return Err(e);
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::net::TcpListener;
    use std::sync::mpsc;

    /// Sobe um listener local e devolve (porta, thread que valida os comandos).
    /// A thread mantém a conexão aberta e envia cada linha recebida pelo canal
    /// (espelha o comportamento do console real: conexão persistente, um
    /// comando por linha).
    fn spawn_listener() -> (u16, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = listener.accept() {
                let mut buf = Vec::new();
                let mut byte = [0u8; 1];
                // Lê até o fim da conexão, uma linha por vez.
                loop {
                    match s.read(&mut byte) {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {
                            buf.push(byte[0]);
                            if byte[0] == b'\n' {
                                let _ = tx.send(String::from_utf8_lossy(&buf).into_owned());
                                buf.clear();
                            }
                        }
                    }
                }
            }
        });
        (port, rx)
    }

    #[test]
    fn connects_and_sends_command_line() {
        let (port, rx) = spawn_listener();
        let mut client = Ma2TcpClient::new(TcpConfig {
            ip: "127.0.0.1".into(),
            port,
        });
        client.connect().unwrap();
        assert!(client.is_connected());

        client.send_command("Go Executor 1").unwrap();
        let received = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(received, "Go Executor 1\n");

        client.send_command("Pause Executor 2").unwrap();
        let received = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(received, "Pause Executor 2\n");
        client.disconnect();
    }

    #[test]
    fn connect_refused_returns_error() {
        // Porta fechada: conecta em um listener já encerrado.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let mut client = Ma2TcpClient::new(TcpConfig {
            ip: "127.0.0.1".into(),
            port,
        });
        assert!(client.connect().is_err());
        assert!(!client.is_connected());
    }

    #[test]
    fn send_without_connect_fails() {
        let mut client = Ma2TcpClient::new(TcpConfig::default());
        assert!(client.send_command("Go Executor 1").is_err());
        assert!(!client.is_connected());
    }

    #[test]
    fn send_failure_marks_disconnected() {
        // O servidor aceita e fecha na hora: o envio falha e o cliente
        // reflete o estado real (desconectado).
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            if let Ok((s, _)) = listener.accept() {
                let _ = s.shutdown(std::net::Shutdown::Both);
            }
        });

        let mut client = Ma2TcpClient::new(TcpConfig {
            ip: "127.0.0.1".into(),
            port,
        });
        client.connect().unwrap();
        // Primeiro envio pode entrar no buffer; repete até o RST chegar.
        let mut failed = false;
        for _ in 0..10 {
            if client.send_command("Go Executor 1").is_err() {
                failed = true;
                break;
            }
        }
        assert!(failed, "envio deveria falhar após o servidor fechar");
        assert!(!client.is_connected());
    }
}