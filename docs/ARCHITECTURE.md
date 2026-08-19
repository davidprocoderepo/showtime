# Arquitetura do Showtime

Guia definitivo de arquitetura para agentes de IA que vão dar continuidade ao
desenvolvimento. **Leia este documento + `AGENTS.md` antes de qualquer edição.**

> Status: **greenfield em produção** — v0.1.0 publicado (GitHub Release), CI
> verde no Windows x64, 49 testes unitários passando, clippy sem warnings.

---

## 1. Visão geral

Aplicativo desktop **Rust** que marca cues de luz sincronizadas com música e
exporta/envia para consoles **GrandMA2** (MA Lighting). A UI é em português;
identificadores de código (crates, comandos) permanecem em inglês.

**Stack pinada** (qualquer substituição exige justificativa):

| Camada | Crate | Versão | Nota |
|---|---|---|---|
| UI | `eframe` / `egui` | 0.36.1 | API muda rápido; ignorar tutoriais antigos |
| Decode | `symphonia` | 0.6.1 | features `mp3,flac,wav,aiff,ogg,pcm` |
| Playback | `rodio` | 0.22.2 | `DeviceSinkBuilder::open_default_sink()` + `Player::connect_new(&sink.mixer())` |
| MIDI out | `midir` | 0.11.0 | Virtual ports NÃO funcionam no Windows |
| MIDI file | `midly` | 0.5.3 | exportação `.mid` |
| Rede | `tokio` | 1.53.1 | **sem feature `macros`** — usar `Runtime::block_on` |
| Serialização | `serde` + `serde_json` + `yaml_serde` | 0.10.6 | **NUNCA** `serde_yaml` (arquivado) nem `serde_yml` (RUSTSEC-2025-0068) |
| Dialogs | `rfd` | 0.17.2 | FileDialog nativo |
| Erros | `thiserror` + `anyhow` | — | domínio vs. orquestração |
| Logging | `log` + `env_logger` | — | init em `main.rs` |

---

## 2. Layout do código

```
src/
├── main.rs               # Entry + guarda Windows-x64-only (compile_error!)
├── error.rs              # ShowtimeError (thiserror) — erros de domínio
├── audio/                # CORE — decode, waveform, playback
│   ├── decoder.rs        #   symphonia → PCM f32 interleaved (UMA vez)
│   ├── waveform.rs       #   picos min/max por bloco (função pura)
│   └── playback.rs       #   rodio transport (play/pause/stop/seek/volume)
├── markers/              # CORE — modelo + CRUD
│   ├── model.rs          #   Marker, MarkerType (Go/Pause/Toggle/Goto/Load)
│   └── manager.rs        #   MarkerManager (ids automáticos, ordenação)
├── timecode/             # CORE — SMPTE 24/25/30 e 29.97 drop-frame
│   ├── model.rs          #   struct Timecode (HH:MM:SS:FF) + parse/display
│   └── conversion.rs     #   seconds↔Timecode com fps/offset/drop-frame
├── export/               # CORE — formatos de saída (funções puras)
│   ├── csv.rs            #   RFC 4180
│   ├── xml.rs            #   estrutura genérica de markers
│   ├── ma2_script.rs     #   Store/Assign + macro XML importável
│   └── midi_file.rs      #   formato 0, 120 BPM, 480 ticks/beat
├── live/                 # CORE — modo ao vivo (GrandMA2)
│   ├── mtc.rs            #   MIDI Time Code (quarter-frames, thread dedicada)
│   ├── midi_events.rs    #   Note On/Off por cue (mapa de notas)
│   └── tcp_client.rs     #   TCP não-oficial, um comando por linha
├── project/              # CORE — persistência
│   ├── model.rs          #   Project (serializável)
│   └── io.rs             #   JSON/YAML (yaml_serde)
└── ui/                   # UI (única camada que importa egui)
    ├── app.rs            #   ShowtimeApp (eframe::App) — orquestração
    ├── timeline.rs       #   waveform + markers + cursor, zoom/seek/drag
    ├── marker_panel.rs   #   lista lateral + diálogo de edição
    ├── transport.rs      #   barra play/pause/stop/seek/volume/timecode
    └── settings.rs       #   fps, drop-frame, offset, MIDI, TCP
```

---

## 3. Regra de ouro (arquitetura)

**O core (`audio`, `markers`, `timecode`, `export`, `project`, `live`) NÃO
pode importar `egui`.** A UI chama o core; nunca o contrário.

- Funções puras e testáveis no core (`cargo test` sem hardware).
- A UI orquestra: thread de fundo para decode, estado, diálogos.
- Erros do core fluem como `ShowtimeError`; a UI os converte em mensagem.

---

## 4. Fluxos de dados

### 4.1 Carregar áudio (o caminho mais usado)

```
UI (botão "Abrir áudio..." / menu Arquivo)
  → ShowtimeApp::open_audio()          [rfd::FileDialog → PathBuf]
  → ShowtimeApp::load_audio(path)      [spawn thread de fundo]
      thread: decoder::decode_file(&path)   → DecodedAudio (PCM f32 interleaved)
              compute_peaks(&samples, 1024, channels) → Waveform
              tx.send((LoadedAudio, Waveform))        [mpsc channel]
  → ShowtimeApp::poll_decode()         [chamado a cada frame, try_recv]
      Ok  → Playback::new(samples, rate, ch) + guarda waveform/duration
      Err → self.error = Some(msg)     [janela de erro]
```

Pontos críticos:
- **Decodifica UMA vez** para `Vec<f32>` e reutiliza para waveform + playback.
- **Nunca fazer I/O na thread da UI** — o decode roda em `std::thread::spawn`.
- `Playback::new` tenta abrir o dispositivo; se falhar, opera em **modo
  silencioso** (relógio avança, sem som) — a UI funciona sem áudio.
- **Gotcha CI**: `open_default_sink()` no Windows headless (runner do GitHub)
  dá **access violation** dentro do cpal — por isso os testes usam o
  construtor `#[cfg(test)] Playback::new_silent()`, que nunca toca o hardware.

### 4.2 Reprodução e relógio

- `Playback` é **self-timed**: `position_sec()` avança pelo `Instant` real
  quando `playing`, independente do dispositivo (modo silencioso incluído).
- Seek não clona amostras: `PcmSource` é um iterador sobre `Arc<Vec<f32>>`
  compartilhado, com `offset` = primeira amostra.
- A UI pergunta `position_sec()` a cada frame; se `playing`, pede repaint
  contínuo (`ctx.request_repaint()`).

### 4.3 Timecode (core, puro)

```
segundos (f64) → real_frames(n = seconds*fps) → nominal_frames(D = n + 2*E(n))
               → nominal_to_tc → add_tc(offset)   [seconds_to_timecode]
```

- Base sempre em **segundos f64**; frame rate é 24/25/30/29.97.
- **Drop-frame 29.97**: pula os frames de contagem 00 e 01 a cada minuto,
  exceto a cada 10º minuto. Implementado por construção com fronteiras de
  minuto (`B_k = round(k*60*29.97)`) e inversa por ponto fixo.
- `offset` (HH:MM:SS:FF) é somado ao resultado (portador = horas).
- Testes incluem **vetores canônicos da indústria** (ex.: `01:00:00:00`
  drop-frame = 107892 frames reais) — não quebrar.

### 4.4 Marcadores

- `MarkerManager` atribui IDs automaticamente e mantém ordem por `time_sec`.
- `Marker.timecode` é **campo calculado** (de `time_sec`, fps, offset) — a UI
  chama `recompute_marker_timecodes()` quando fps/drop/offset mudam. É mantido
  em disco para round-trip exato do projeto.
- Ações da timeline: duplo clique adiciona, clique direito remove, arrasto =
  seek, scroll = zoom ancorado no cursor, Shift+scroll = rolagem.

### 4.5 Modo ao vivo

```
Playback.position_sec()
  ├── MtcSender.set_position_sec()   → thread MTC (240 msg/s a 30fps)
  │      converte pos → Timecode → 8 quarter-frames (0xF1) do frame atual
  ├── MidiEventSender.send(marker)   → Note On/Off (mapa por MarkerType)
  └── Ma2TcpClient.send_command()    → "Go Executor N" etc. (TCP, block_on)
```

- `fire_live_events(last_pos, pos)` dispara eventos dos marcadores cruzados no
  intervalo — só quando `pos > last_pos` (nunca em rewind).
- Thread MTC: `thread::Builder::name("showtime-mtc")`, sleep compensado por
  `Instant` para evitar deriva; `stop` via `AtomicBool`; `Drop` faz join.

### 4.6 Persistência

- `Project` serializa para JSON ou YAML (extensão decide). YAML via
  `yaml_serde` (drop-in de `serde_yaml` — os `use serde_yaml::` continuam
  funcionando porque o crate se chama `serde_yaml` no Cargo).
- Ao abrir projeto com `audio_file_path`, o app recarrega o áudio se o arquivo
  existir; senão mostra erro (não falha o carregamento do projeto).

---

## 5. Padrões e convenções

1. **Strings da UI em português**; nomes de crates/comandos em inglês.
2. **Erros**: `ShowtimeError` (thiserror) no core; `anyhow` só em orquestração.
3. **Thread-safety**: UI em thread única; `MarkerManager` não é `Send` —
   não compartilhar entre threads sem `Mutex`.
4. **Tokio sem macros**: `Runtime::new()` + `block_on` (ver `tcp_client.rs`).
5. **Testes sem hardware**: playback com `new_silent()`, waveform com dados
   sintéticos, MIDI testa só a codificação de nibbles (função pura).
6. **Decode único**: PCM f32 → `Arc<Vec<f32>>` compartilhado.
7. **Commits atômicos** com prefixo `GIT_MASTER=1` e footer
   "Ultraworked with [Sisyphus]..." + "Co-authored-by: Sisyphus".

---

## 6. Gotchas conhecidos (histórico real)

| Problema | Causa | Solução aplicada |
|---|---|---|
| Segfault no CI (0xc0000005) | cpal/WASAPI sem dispositivo de áudio no runner Windows | Construtor `#[cfg(test)] Playback::new_silent()` nos testes |
| `brew install mingw-w64` falha no macOS 13 | sem bottle; build do cmake quebra com CLT 14.3.1 | llvm-mingw (ver §7) |
| Linker `x86_64-w64-mingw32-gcc` not found | `libgcc.a`/`libgcc_eh.a` ausentes no llvm-mingw | symlinks → `libclang_rt.builtins-x86_64.a` e `libunwind.a` |
| `serde_yaml`/`serde_yml` | arquivado / RUSTSEC-2025-0068 unsound | `yaml_serde` 0.10.6 |
| egui mudou API (OutputStream/Sink antigos) | versões recentes | usar API rodio 0.22 (ver AGENTS.md) |
| MTC deriva | sleep não compensado | `Instant` + `next_frame += frame_dur` |

---

## 7. Build, teste e release

### Local (macOS dev)
```sh
cargo test          # 49 testes — a guarda cfg(test) libera qualquer SO
cargo clippy --all-targets   # 0 warnings (obrigatório antes de commit)
```

### Windows nativo (CI)
`.github/workflows/ci.yml`: runner `windows-latest` → `cargo build --release`
+ `cargo test --release` + upload do `showtime.exe`; job de release em tags
`v*` cria a GitHub Release com o binário e os créditos.

### Cross-compile macOS → Windows x64 (manual)
Requires llvm-mingw em `/tmp/llvm-mingw-20260616-ucrt-macos-universal`:
```sh
# 1. reaplicar symlinks (são relativos ao toolchain extraído):
TC=/tmp/llvm-mingw-20260616-ucrt-macos-universal
ln -sf "$TC/lib/clang/22/lib/windows/libclang_rt.builtins-x86_64.a" "$TC/x86_64-w64-mingw32/lib/libgcc.a"
ln -sf "$TC/x86_64-w64-mingw32/lib/libunwind.a" "$TC/x86_64-w64-mingw32/lib/libgcc_eh.a"
# 2. PATH com o bin/ do toolchain (precisa do dlltool)
export PATH="$TC/bin:$PATH"
# 3. build (linker/rustflags vêm de .cargo/config.local.toml — gitignored)
cargo build --release --target x86_64-pc-windows-gnu
```
> O `/tmp` é limpo pelo macOS: se o toolchain sumir, rebaixar
> `https://github.com/mstorsjo/llvm-mingw/releases/download/20260616/llvm-mingw-20260616-ucrt-macos-universal.tar.xz`
> e reaplicar os symlinks.

### Release
```sh
git tag v0.2.0 && git push origin v0.2.0   # CI publica automaticamente
```

---

## 8. Guia prático para agentes

### Antes de editar
1. Leia `AGENTS.md` (regras do repo) + este documento.
2. Não toque em `.omo/` nem `.codegraph/` (artefatos de tooling).
3. Use `codegraph_explore` para entender símbolos antes de ler arquivos.

### Para adicionar uma feature
1. Identifique a camada: é core puro (função) ou UI (orquestração)?
2. Core puro → implemente com teste unitário (sem hardware).
3. UI → chame o core; nunca importe egui dentro do core.
4. Rode `cargo test` + `cargo clippy --all-targets` (0 warnings).
5. Verifique o cross-build Windows se a mudança tocar o core.
6. Commit atômico com `GIT_MASTER=1` (footer Sisyphus) e push.

### Armadilhas frequentes
- **Não decodifique o áudio duas vezes** — reutilize o `Arc<Vec<f32>>`.
- **Não bloqueie a thread da UI** com decode/I/O — use thread de fundo + canal.
- **Não use `#[tokio::main]`** — tokio sem feature `macros`.
- **Não importe egui no core** — quebra a regra de ouro (e o teste de clippy).
- **Não use `serde_yaml`/`serde_yml`** — `yaml_serde`.
- **Testes não podem abrir dispositivo de áudio** — `new_silent()`.

### Critérios de aceite do projeto (spec §10)
compila `--release`; carrega WAV/MP3/FLAC com waveform; transport com seek;
CRUD de marcadores na timeline; timecode correto por fps/offset (testes);
export CSV/XML; salva/carrega projeto; MTC via MIDI se dispositivo disponível;
UI responsiva em arquivos 1h+.

---

## 9. Estado atual (checkpoint)

- ✅ v0.1.0 publicado: https://github.com/davidprocoderepo/showtime/releases/tag/v0.1.0
- ✅ CI verde (build+test Windows x64, release automática em tags `v*`)
- ✅ Guarda `compile_error!` Windows-x64-only (cfg(test) libera dev em qq SO)
- ✅ Botão "Abrir áudio..." (menu Arquivo) + placeholder na timeline
- ✅ 49/49 testes, clippy 0 warnings
- ⏳ Próximos passos naturais: refinamentos de UI, validação do DTD do macro
  MA2 contra um console real, testes de integração do modo ao vivo com
  loopback MIDI.