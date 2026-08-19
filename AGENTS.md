# AGENTS.md — Showtime (marcador de cues para GrandMA2)

Instruções para agentes trabalhando neste repositório. Cada linha responde: "um agente erraria isso sem ajuda?" Se não, não está aqui.

## Status
- Projeto **em produção**: v0.1.0 publicado, CI verde (Windows x64), 49 testes passando, clippy 0 warnings.
- **Leia também** [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — guia completo de módulos, fluxos de dados, gotchas e desenvolvimento.
- **Não tocar** em `.omo/` (artefatos de plano) nem `.codegraph/` (índice local de tooling).
- Repositório **git ativo** (origin: `https://github.com/davidprocoderepo/showtime.git`): commits atômicos com `GIT_MASTER=1` e push em `main`. CI roda a cada push; releases automáticas em tags `v*`.
- A UI é em português; nomes de crates e comandos permanecem em inglês.
- **Plataforma**: Windows x64 apenas (guarda `compile_error!` em `main.rs`); `cargo test` roda em qualquer SO.

## Stack (pinada — qualquer substituição exige justificativa no PR)
- UI: `eframe`/`egui` — use a **última versão**; a API do egui muda rápido, tutoriais antigos (OutputStream/Sink etc.) estão defasados.
- Decodificação: `symphonia` (features `mp3,flac,wav,aiff,ogg,pcm`; suporta todos os formatos da spec).
- Playback: `rodio` — API atual: `DeviceSinkBuilder::open_default_sink()` + `Player::connect_new(&sink.mixer())` + `player.append(source)`. O decoder padrão do rodio já é symphonia-backed.
- MIDI out: `midir` (v0.11+, ativo em 2026). Backends: ALSA/WinMM/CoreMIDI (+ `winrt`/`jack` por feature). **Virtual ports não funcionam no Windows.**
- Rede: `tokio` (`tokio::net`) para TCP; `reqwest` só se precisar de HTTP.
- Serialização: `serde` + `serde_json` + **`yaml_serde`**.
  - **NUNCA** use o crate `serde_yaml` original (arquivado/sem manutenção) nem `serde_yml` (RUSTSEC-2025-0068, unsound).
  - Migração drop-in: `serde_yaml = { package = "yaml_serde", version = "0.10" }` mantém os `use serde_yaml::`.
- Dialogs: `rfd`. Erros: `anyhow` (gerais) + `thiserror` (domínio, em `src/error.rs`). Logging: `log` + `env_logger`. Arquivo .mid (exportação): `midly`.

## Arquitetura
- Módulos em `src/`: `audio/` (decoder, playback, waveform), `markers/` (model, manager), `timecode/` (struct + conversion), `export/` (csv, xml, ma2_script, midi_file), `live/` (mtc, midi_events, tcp_client), `project/` (model, io), `ui/` (app, timeline, marker_panel, transport, settings), `error.rs`. Entry: `src/main.rs` → `src/ui/app.rs` (`eframe::App`).
- **Regra de ouro**: o core (`audio`, `markers`, `timecode`, `export`, `project`, `live`) **não pode importar `egui`**. A UI chama o core; traits (`AudioSource`, `MidiOutput`) abstraem backends.
- Funções puras para conversão de timecode e cálculo de waveform (testáveis com `cargo test`, sem hardware).

## Gotchas de implementação
- **Decodifique UMA vez** para PCM f32 e reutilize: waveform (picos min/max por bloco de 1024 amostras) + playback via `SamplesBuffer`/`Source` custom. Nunca decodificar duas vezes.
- Arquivos 1h+: f32 estéreo 44.1kHz ≈ **1.27 GB/hora em RAM**. Para arquivos longos: downmix mono/i16 e waveform lazy. **Nunca faça I/O bloqueante na thread da UI** — decode em thread de fundo + canal.
- Timecode: suportar 24/25/30 e **29.97 drop-frame**. Drop-frame pula os frames 00 e 01 a cada minuto, exceto a cada 10º minuto. Base = amostras → segundos `f64`. Offset HH:MM:SS:FF aplicado ao início da música.
- MTC (modo ao vivo): 8 mensagens quarter-frame (`0xF1`) por frame SMPTE; a 30fps = **240 msg/s**. Thread dedicada (alta prioridade) sincronizada com o clock do áudio; usar pré-roll para compensar latência/buffer.
- TCP para MA2 é **não-oficial** — um comando por linha (`Go Executor 1`), IP/porta configuráveis; documentar que depende do console aceitar.

## Integração GrandMA2
- Macros MA2 = linhas de comando em `.xml`: `<Macro name="..."><MacroLine command="Store Executor 1 Cue 1"/></Macro>`. Params por linha: `CMD`, `Wait`, `Info`, `Disabled`. **Validar o DTD exato contra um macro exportado pelo console** (formato pode variar). Import no console: Setup → Import/Export → Import → Macro.
- Comandos típicos: `Go Executor N`, `Pause Executor N`, `Goto Cue N Executor N`, `Store Executor N Cue N`, `Assign Timecode HH:MM:SS:FF Executor N Cue N`.
- Export CSV: `timecode, cue_number, executor, tipo, nome, comentario`.

## Workflow
- Build: `cargo build --release`. Testes: `cargo test` (unitários nos módulos puros — conversão de timecode, waveform com dados sintéticos).
- Ordem de implementação (spec §10): timecode → modelos → decode → waveform → playback → markers CRUD → UI básica → edição → export CSV/XML → settings → MIDI/MTC → modo ao vivo → persistência → polimento.
- Critérios de aceite: compila `--release`; carrega WAV/MP3/FLAC com waveform; transport com seek; CRUD de marcadores na timeline; timecode correto por fps/offset (testes); export CSV/XML; salva/carrega projeto; MTC via MIDI se dispositivo disponível; UI responsiva em arquivos 1h+.