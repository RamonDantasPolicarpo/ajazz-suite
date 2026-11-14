# Estado Atual do Projeto (CONTEXT.md)

Este documento registra a posição exata do projeto, decisões tomadas, descobertas de hardware e o próximo passo imediato para que qualquer retomada de sessão seja instantânea e sem perda de contexto.

---

## 1. Posição Atual: Mouse AJ179 NL P Integrado Fim-a-Fim (Protocol + Daemon + GUI) — Aguardando Validação de Escrita no Hardware

- **Mouse AJAZZ AJ179 NL P (`NearLink BS25/BS2X`, `AJAZZ Driver（W1）.exe`):**
  - Política anti-brick e bloqueio de DFU/Bootloader registrada em [ADR-0003](docs/adr/0003-mouse-anti-brick-and-dfu-lockout.md) (`UsagePage 0xFFB1`, `BurnTool.exe`, `libburn.dll` e `06 0F ... FF` estritamente bloqueados).
  - Protocolo HID proprietário documentado em [mouse_protocol.md](docs/protocol/mouse_protocol.md) (`VID 0x7768`, `PID 0x642E` 2.4G / `0x6D2E` USB, `UsagePage 0xFFB2 / Usage 0x01`, reports de 64 bytes) e implementado em `crates/ajazz-protocol/src/mouse.rs` (`Aj179MouseDevice`, `MouseStatus`, `MouseConfig`, `MouseButtonSlot`, `MouseProfile`, validado por leitura real no hardware em 2.4G: FW `1.0.14`, bateria `50%`, 6 estágios DPI).
  - **Daemon (`ajazz-daemon`)**: IPC estendido com `GetMouseStatus`, `SetMouseConfig`, `SetMouseButton`, `ResetMouseButtons`, `SyncMouseFromDevice` e `StartMousePairing` (`IpcResponse::MouseStatus`); detecção automática/hot-unplug, aplicação por diff de parâmetros, drenagem de notificações assíncronas `0xFA` e persistência em `UserProfile.mouse`.
  - **Interface Gráfica (`ajazz-gui`)**: Alinhamento concluído via **Grill Me** (2 rodadas) e aba `AJ179 NL P` implementada (`crates/ajazz-gui/ui/mouse_view.slint`, `crates/ajazz-gui/src/mouse_ui.rs`, `sidebar.slint`, `ipc_client.rs`) com telemetria global de bateria/conexão na Sidebar, barra fixa de telemetria, 3 sub-abas (`Performance & Sensor` com 6 estágios DPI/Polling/LOD/Sensor/Debounce, `Botões` com esquema visual dos 5 botões + trava anti-lockout do Clique Esquerdo, `Energia & Conexão` com presets de Sleep/Scroll/Pareamento 2.4G em 2 cliques) e modo rascunho + `Aplicar no Mouse` / `Sincronizar`. Todos os 38 testes do workspace passam.

- **Protocolo e Engenharia Reversa (`docs/protocol/hid_protocol.md` + `ajazz-protocol`):**
  - **`SystemStages` (`Opcode 0x05` / `0x06`)**: `HID[27]=WFlag`, `HID[28]=Key6Flag`, `HID[29]=WinFlag`, `HID[30]=PollingRate` (`0..3` -> `1000/500/250/125 Hz`), `HID[35]=MacMode`, `HID[36]=Wheel`. Configurado de forma unificada (`build_config_packet` / `parse_config_block`) para preservar iluminação e flags de sistema juntos.
  - **Keymap (`Opcode 0x03`, `0x1B`, `0x07`, `0x09`)**: Descoberta dinâmica de `total_keys` e blocos de macro (`0x03`), mapa `led_map[hw_key_idx] == phys_led` (`0x1B`), leitura base da `KeyMatrix` (`0x07`), registros de 3 bytes Big-Endian (`Tecla/Combo`, `Mouse`, `Mídia/Consumer`, `Sistema ACPI`, `Macro 0x70/0x71`, `Camada`, `Fn`, `Desabilitar`) com `AK820_FACTORY_ASSIGNMENTS` (82 teclas) e escrita fechada em transação (`0x01 -> 0x09 -> 0x02`).
  - **Macros (`Opcode 0x15`)**: Contêiner binário `0x55AA` LE empacotado com tabela de offsets e eventos de 4 bytes (`Type 10` HID, `9` Modificador, `1` Botão Mouse, `4/5` Movimento X/Y relativo).
  - **Audio Sync (`crates/ajazz-protocol/src/audio.rs`, Modo `0x1D -> 0xFE`)**: Pipeline zero-alloc derivado do `AudioRecord.exe` + executável oficial (`FFT radix-2 1024`, 25 bandas espectrais com fluxo/desvio-padrão, decaimento por tecla de `sub_4b4c70`, 7 estilos `0..=6`: `VerticalBars`, `RainbowPulse`, `CenterWave`, `ConcentricRings`, `PeakDrop`, `HorizontalBars`, `ZigZagBars` e fundo configurável).
- **Daemon (`ajazz-daemon`):**
  - `UserProfile` estendido com `system`, `keymap`, `macros` e campos `audio_style`/`audio_bg_enabled`/`audio_bg_color` retrocompatíveis (`#[serde(default)]`).
  - Capturador WASAPI loopback nativo (`crates/ajazz-daemon/src/audio_capture.rs`, COM FFI zero dependências extras e buffers fixos `[f32; 1024]`) enviando quadros via `Opcode 0x12` (~30 FPS, skip em silêncio estável).
  - `PlayGif` / `StopGif` / `RenderFrame` reativados usando `0xFE` uma única vez no início e streaming em RAM volátil via `Opcode 0x12` (zero escrita flash `0x0B`/`0x02` em loop), restaurando o efeito anterior no `StopGif`.
- **Interface Gráfica Nativa (`ajazz-gui`):**
  - Sub-abas em `AK820 RGB` (`Iluminação`, `Remapear Teclas`, `Macros`, `Sistema`) mantendo o teclado 82 teclas fixo no topo (`980x680` travado) com destaque de seleção e ponto indicador de remapeamento.
  - Modo `Audio` na 21ª vaga da grade `7×3` com linha contextual dos 7 estilos + toggle/cor de fundo, e barra compacta `GIF LIVE` (seletor nativo Windows `GetOpenFileNameW`, FPS `15/25/30` e `PLAY/PARAR GIF` sincronizado com o Daemon).
- **Validação de Hardware Concluída:** `Audio Sync` (7 estilos + fundo) e `GIF Player (0x12)` aprovados pelo usuário no AK820 físico (paridade total do teclado concluída).

---

## 2. Inventário de Hardware Identificado (Evidência Direta)

| Dispositivo | Fabricante | VID | PID | Endpoint de Controle Proprietário |
| :--- | :--- | :--- | :--- | :--- |
| **Teclado AK820** | `Evision` | `0x320F` | `0x505B` | `UsagePage=0xFF1C, Usage=0x0092` |
| **Mouse NearLink (AJ179 NL P)** | `AJAZZ` | `0x7768` | `0x642E` (2.4G) / `0x6D2E` (USB) | `UsagePage=0xFFB2, Usage=0x01` (`0xFFB1` DFU bloqueado) |

---

## 3. O Que Já Foi Entregue (Histórico de Commits)

- `58779a1`: `docs: setup architectural docs, domain glossary and agent guidelines`
- `e284341`: `feat: initialize cargo workspace with protocol, daemon, and slint gui crates`
- `8f8baf2`: `feat: identify physical AK820 and NearLink mouse VID/PID and vendor usage pages`
- `e547c98`: `docs: document hardware endpoints and vendor usage pages for AK820 and NearLink mouse`
- `4248d45`: `chore: add docs captures directory for usb traffic traces`
- `5b707b3`: `feat: decode AK820 lighting protocol and implement static color control in Rust`
- `f55ddd7`: `docs: add anti-brick and flash wear-out safety rules`
- `034df4c`: `docs: document comprehensive GUI architecture and design specification from grill-me session`
- `1ddf438`: `docs: adjust GUI palette to graphite-neutral dark tone and JetBrains typography`
- `e7999c2`: `feat: implement headless ajazz-daemon with Windows Named Pipe IPC and hardware actor`
- `a282dd7`: `feat: implement native Slint GUI with 75% AK820 layout, monochromatic graphite theme and IPC client`
- `be2fad5`: `fix: correct AK820 75% physical keycap alignment, arrow cluster and navigation column`
- `5ace228`: `fix: embed JetBrains Mono font, fix badge padding and align keyboard and sidebar geometry`
- `2c8663c`: `feat: implement custom per-key rgb matrix, 25 fps gif player, and native gui controls`
- `112df72`: `chore: disable gif streaming due to hardware flicker and remove gif tab from gui`
- `5cac230`: `docs: align domain glossary, adr-0002 and lighting parity requirements`
- `3c5ed87`: paridade completa do subsistema de iluminação AK820 (19 efeitos + Custom 3 bancos + Capture `0x12` + persistência JSON + GUI travada).
- `ab78f2f`: `feat: implement AK820 SystemStages, keymap remapping and macro editor` (paridade completa do teclado AK820: iluminação + sistema + keymap + macros validados no hardware).
- `e306074`: `feat: implement Audio Sync (0x1D) and volatile GIF Player (0x12)` (Audio Sync WASAPI 7 estilos + GIF Player em RAM volátil validados no hardware).
- `9159a4c`: `docs: record hardware validation for Audio Sync and GIF Player`
- `b6574aa`: `chore: ignore AJAZZ_Mouse official software directory`
- `dcacc6d`: `docs: document AJ179 NL P HID protocol and anti-brick ADR`
- `dbcfe99`: `feat(protocol): implement AJ179 NL P HID codecs and validated status reader`
- `a4a0590`: `feat(daemon): integrate AJ179 NL P mouse IPC, state sync, and hot-unplug handling`

---

## 4. Próximos Passos Imediatos

1. **Validação ao Vivo no Mouse Físico (`AJ179 NL P`)**: executar `ajazz-daemon` + `ajazz-gui` e validar escrita ao vivo no hardware (DPI `50..26000`, troca de estágio, LOD, sensor flags, polling rate, debounce, sleep e remapeamento dos 5 botões).
2. **Refinamentos do GIF Player (pós-consolidação)**: adicionar enquadramento/recorte (*crop* / *pan & zoom*) da região do GIF projetada na matriz e controle fino de velocidade/FPS mais baixos (1–10 FPS / multiplicador).
3. **Macros no Mouse (`06 31`) + Detector de Switch Chattering**: adicionar suporte de macros do mouse (`06 31`) caso necessário após validação e diagnóstico de chattering.
