# Showtime

Marcador de cues de luz sincronizadas com música, com exportação e envio direto para consoles **GrandMA2** (MA Lighting).

![CI](https://github.com/davidprocoderepo/showtime/actions/workflows/ci.yml/badge.svg)

> **Plataforma**: Windows **x64** apenas. O binário é travado em tempo de compilação para
> `x86_64-pc-windows-*` (MSVC ou GNU); `cargo test` continua rodando em qualquer SO.

App desktop em Rust (`eframe`/`egui`). Decodifica áudio (WAV/MP3/FLAC/AIFF/OGG), desenha a waveform, reproduz com seek, marca cues na timeline com timecode SMPTE (24/25/30 e 29.97 drop-frame) e exporta para CSV, XML, macro MA2 e arquivo `.mid` — além de envio ao vivo por MTC (MIDI), eventos MIDI e TCP para o console.

## Créditos

Criado e desenvolvido por **david.luz.led** — Instagram: [@david.luz.led](https://instagram.com/david.luz.led)

## Funcionalidades

- **Áudio**: decode com `symphonia` (uma vez, para PCM f32) reutilizado para waveform + playback com `rodio`. Seek sem clonar amostras (source sobre `Arc<Vec<f32>>`).
- **Timecode SMPTE**: 24, 25, 30 e **29.97 drop-frame** (pula frames 00/01 por minuto, exceto a cada 10º minuto), com offset `HH:MM:SS:FF` aplicado ao início da música.
- **Marcadores (cues)**: CRUD na timeline (duplo clique adiciona, clique direito remove, arrastar = seek), tipo (Go/Pause/Goto/Toggle/Load), executor e número de cue.
- **Exportação**:
  - **CSV**: `timecode,cue_number,executor,tipo,nome,comentario`
  - **XML**: estrutura de markers
  - **Macro MA2** (`.xml`): `<Macro name="..."><MacroLine command="Store Executor N Cue M"/></Macro>` — importável via Setup → Import/Export → Import → Macro
  - **Arquivo `.mid`** (`midly`): nota por cue (delta 960 ticks, 120 BPM)
- **Ao vivo**:
  - **MTC** (MIDI Time Code): 8 quarter-frames por frame SMPTE (240 msg/s a 30fps) em thread dedicada, sincronizado com o clock do áudio.
  - **Eventos MIDI** (saída por `midir`): dispara nota ao cruzar cada cue.
  - **TCP para GrandMA2** (não-oficial): um comando por linha — `Go Executor N`, `Pause Executor N`, `Goto Cue N Executor N`. IP/porta configuráveis; depende do console aceitar a conexão.
- **Projeto**: salvar/carregar em **JSON** ou **YAML** (via `yaml_serde` — fork mantido do `serde_yaml`).

## Build

```sh
cargo build --release
# binário em target/release/showtime
```

Testes unitários (conversão de timecode, waveform com dados sintéticos, exportação):

```sh
cargo test
```

## Uso

1. **Arquivo → Abrir áudio** e carregue a música (WAV/MP3/FLAC/AIFF/OGG). A waveform é computada em thread de fundo — a UI nunca bloqueia.
2. Ajuste **frame rate / drop-frame / offset** no menu **Configurações**.
3. Toque/pause (teclas de transporte), arraste na timeline para seek.
4. Duplo clique na timeline para **adicionar cue** na posição atual; clique direito remove; clique no marcador seleciona; botão **Editar** ajusta nome/tipo/executor/cue.
5. **Arquivo → Exportar** escolha o formato (CSV, XML, Macro MA2, .mid).
6. **Configurações → Conexão GrandMA2**: configure IP/porta e conecte para envio TCP ao vivo; selecione dispositivo MIDI para MTC/eventos.

## Arquitetura

> Documentação completa para agentes de IA e novos desenvolvedores:
> **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)** — módulos, fluxos de dados,
> decisões de design, gotchas e guia de desenvolvimento.

```
src/
├── audio/       # decoder (symphonia), waveform (picos min/max por bloco), playback (rodio)
├── markers/     # modelo de marker + manager (ids, ordenação)
├── timecode/    # struct Timecode + conversões NDF/DF com offset
├── export/      # csv, xml, ma2_script, midi_file
├── live/        # mtc (quarter-frames), midi_events, tcp_client (MA2)
├── project/     # modelo de projeto + io (JSON/YAML)
├── ui/          # app (eframe::App), timeline, marker_panel, transport, settings
├── error.rs     # ShowtimeError (thiserror)
└── main.rs      # entry (eframe::run_native)
```

**Regra de ouro**: o core (`audio`, `markers`, `timecode`, `export`, `project`, `live`) não importa `egui`; a UI chama o core. Trait `AudioSource` abstrai a fonte de áudio; trait `MidiOutput` abstrai o backend MIDI.

## Integração GrandMA2

- **Macros MA2** são linhas de comando em `.xml`: `<Macro name="..."><MacroLine command="Store Executor 1 Cue 1"/></Macro>` (params por linha: `CMD`, `Wait`, `Info`, `Disabled`). Importe em **Setup → Import/Export → Import → Macro**.
- Comandos típicos: `Go Executor N`, `Pause Executor N`, `Goto Cue N Executor N`, `Store Executor N Cue N`, `Assign Timecode HH:MM:SS:FF Executor N Cue N`.
- **TCP é não-oficial** — um comando por linha, IP/porta configuráveis. Validar se o console aceita antes de depender do recurso.

## Observações

- **Virtual ports MIDI não funcionam no Windows** (limitação do backend WinMM); use um driver de loopback real.
- Arquivos longos (1h+): f32 estéreo 44.1kHz ≈ 1.27 GB/hora em RAM; o decode em background evita travar a UI, mas o uso de memória é proporcional ao áudio carregado.