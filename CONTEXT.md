# Estado Atual do Projeto (CONTEXT.md)

Este documento registra a posição exata do projeto, decisões tomadas, descobertas de hardware e o próximo passo imediato para que qualquer retomada de sessão seja instantânea e sem perda de contexto.

---

## 1. Posição Atual: Paridade Core do Teclado AK820 e Mouse AJ179 NL P Validados em Hardware

- **Mouse AJAZZ AJ179 NL P (`NearLink BS25/BS2X`, `AJAZZ Driver（W1）.exe`):**
  - Política anti-brick e bloqueio de DFU/Bootloader registrada em [ADR-0003](docs/adr/0003-mouse-anti-brick-and-dfu-lockout.md) (`UsagePage 0xFFB1`, `BurnTool.exe`, `libburn.dll` e `06 0F ... FF` estritamente bloqueados).
  - Protocolo HID proprietário documentado em [mouse_protocol.md](docs/protocol/mouse_protocol.md) (`VID 0x7768`, `PID 0x642E` 2.4G / `0x6D2E` USB, `UsagePage 0xFFB2 / Usage 0x01`, reports de 64 bytes) e implementado em `crates/ajazz-protocol/src/mouse.rs` (`Aj179MouseDevice`, `MouseStatus`, `MouseConfig`, `MouseButtonSlot`, `MouseProfile`, validado por leitura/escrita real no hardware: FW `1.0.14`, bateria reportada pelo firmware em passos de `10%` — validado `40%` -> cabo `CHG` -> `70%`, DPI, Scroll, LOD, Polling Rate, Ripple Control e Linear Correction/Angle Snapping sincronizando inclusive com o app oficial).
  - **Diagnóstico de Bateria + Polling (`DUI.dll` + `06 05` ao vivo)**:
    - Evidenciado em `DUI.dll` (`BatteryCtrl::DrawControl`) que o app oficial exibe 5 faixas (`<25`, `<50`, `<75`, `<95`, `>=95`) sobre `rx[13]`, enquanto o mouse real reporta `0x32` (`50%`) em `rx[13]`.
    - `ajazz-daemon` passou a re-consultar `06 05` periodicamente a cada 15s (`MOUSE_STATUS_REFRESH_INTERVAL`), preservando bateria/config em cache quando `rx[12] == 0` (sleep, igual a `sub_42f930`) e ignorando `0xFA 03` com `rx[6] == 0` (igual a `sub_434ae0`).
  - **Correções pós-validação no Mouse**:
    - `b690faa`: corrigido bug de polling periódico `MouseStatus` sobrescrevendo rascunhos de botões e DPI na GUI.
    - `6720984`: diagnosticado via `diagnosing-bugs` + `Kernel-Power` Event 506 + `AJAZZ Driver（W1）.exe` (`sub_4247b0` + `sub_415a80`): `[0x40, 0x01/0x02/0x04, 0x00, 0x00]` são **atalhos ACPI de Sistema (`SystemKey`: Power/Sleep/Wake)** via `UsagePage 0x01 / Usage 0x80`, não Sniper DPI (`215/216` em `sub_435ea0` não têm handler em `sub_4209c0` nem opcode no AJ179). `MouseButtonSlot::SniperDpi` substituído por `MouseButtonSlot::System(SystemKey)` (com migração segura de perfis legados `SniperDpi` -> `Disable`) e aba `Sistema` com avisos explícitos.
  - **Macros no Mouse (`06 31`) e Detector de Switch Chattering (`Raw Input`)**:
    - Protocolo `06 31` (`sub_430860` no `AJAZZ Driver（W1）.exe`) documentado em `docs/protocol/mouse_protocol.md` (tabela de 32 offsets `u16_le` `[0x00..0x3F]`, pre-delay no primeiro evento `10 ms`, máscaras `0x41/0x01` modificador vs `0x42/0x02` tecla normal vs `0x43/0x03` botão de mouse, pareamento automático de release, terminador `[10ms, 0x83, 0x01]` + 4 bytes zero, e envio em fatias de 59 bytes `06 31` + vínculo `06 12` `60 <idx> <loop_count> <loop_mode>`) e implementado em `crates/ajazz-protocol/src/mouse.rs` (`compile_mouse_macros_container`, `build_upload_mouse_macros_packets`, `Aj179MouseDevice::upload_macros`).
    - **Fix de loop infinito em macro de mouse (validado em hardware)**: diagnosticado via oráculo binário (`sub_425440` + `sub_415790` em `label_4158b9` + `sub_415200`) que o slot `0x60` do `06 12` usa `[0x60, idx, loop_count, loop_mode]` (e não `[loop_mode, loop_count]`). Para "10x", o código anterior enviava `[0x60, 0x00, 0x00, 0x0A]` (`loop_count = 0` e `loop_mode = 10`), fazendo o MCU repetir indefinidamente na memória flash do mouse mesmo após reiniciar o PC. Corrigidos `MouseButtonSlot::Macro::to_wire()` (incluindo clamp `loop_count >= 1` / `loop_mode <= 2`), `validate_outgoing_packet` em `06 12` (bloqueando `count=0` ou `mode>2`) e `mouse_ui.rs` (`loop_count = 1` padrão nos modos `1`/`2`). Repetição `10x` validada em hardware sem loop infinito.
    - Monitor passivo de **Switch Chattering** (`crates/ajazz-daemon/src/chattering.rs`) via Windows Raw Input (`HWND_MESSAGE` + `RIDEV_INPUTSINK`, zero CPU ocioso, zero alocações no hot path, ignora eventos simulados `h_device == 0`), com sensibilidade persistida em `MouseProfile.chattering` (padrão `45 ms`, `5..=150 ms`).
    - **Interface Gráfica (`ajazz-gui`)**: após alinhamento via **Grill Me**, adicionada categoria `Macro` em `BOTÕES (5)` (reutilizando a biblioteca compartilhada + modos `N vezes`, `Até soltar`, `Alternar` e atalho para `Gerenciar Macros`) e sub-aba `DIAGNÓSTICO` com badge de alerta no cabeçalho, controles de sensibilidade/pausa/zerar e tabela ao vivo por switch físico.
  - **Identidade Visual & SVGs na GUI (`ajazz-gui`)**: após rodada completa de **Grill Me**, criadas a logo SVG exclusiva (`crates/ajazz-gui/ui/assets/logo.svg`, aplicada na sidebar e como `Window.icon`) e biblioteca de ícones SVG locais monocromáticos (sidebar teclado/mouse/configurações, sub-abas do mouse, conexão `USB`/`2.4G`, bateria dinâmica + raio de carregamento substituindo `(CARREG.)`); em `BOTÕES (5)`, substituído o esquema genérico pelos traçados vetoriais reais (`Slint Path`) extraídos de `shape_mouse_cima.svg` (ESQ/DIR/SCROLL) e `shape_mouse_lado.svg` (AVANÇAR/VOLTAR) empilhados, com hover/seleção independente e chips de legenda clicáveis.
  - **Estágios de DPI Coloridos & Sincronização ao Vivo do Botão Físico**: adicionadas bolinhas coloridas fixas nos perfis `S1..S6` (`#EF4444`, `#22C55E`, `#3B82F6`, `#06B6D4`, `#EAB308`, `#EC4899`) com ponto branco central no estágio ativo em `mouse_view.slint` (mantendo barra e botões no tema original); corrigidos `Aj179MouseDevice` (fila `pending_events` para não descartar pacotes `0xFA` durante `06 05`), `poll_or_connect_mouse` (propaga `dpi_index` vindo de `0xFA 0x02` ou `06 05` para `profile.mouse.config.dpi_index`), `merge_mouse_status` (atualiza `ms-dpi-index` mesmo com rascunho `dirty`) e polling da GUI para `300 ms` no mouse.
  - **Fix de Timeout/Aplicação de DPI no Mouse & Auto-Sync de DPI na Conexão (validado em hardware)**:
    - Diagnosticado via captura HID ao vivo + `sub_449170`: (1) `parse_status_response` aceitava pacotes `06 07` residuais como se fossem `06 05` (produzindo `[50; 6]`), (2) o dongle NearLink entra em defasagem de 1 pacote (`lag = 1`) após timeout/resposta pendente e retém a resposta em EP_IN até o próximo `WriteFile`, e (3) `SetMouseConfig` usava timeout IPC curto (`4000 ms`) enquanto a edição de texto de DPI na GUI ignorava valores válidos ou não refletia valores ajustados após `Aplicar`.
    - Corrigidos `crates/ajazz-protocol/src/mouse.rs` (validação estrita de opcode `rx[0..2] == pkt[0..2]` para `0x05`/`0x07`/`0x13`, cache de `06 07` por conexão, retry `10 × (100 ms + 10 ms)` de `sub_449170` com preservação de `0xFA` e até 2 flushes somente-leitura `06 05` para drenar `lag >= 1`), `crates/ajazz-daemon/src/device_manager.rs` (`sync_profile_mouse_from_poll` para auto-sync seguro de DPI/config na conexão inicial e polls online sem sobrescrever com payloads `[50; 6]` offline), `crates/ajazz-gui/src/ipc_client.rs` (timeout de `10_000 ms` para comandos de escrita) e `crates/ajazz-gui/src/mouse_ui.rs` (`parse_dpi_input` + re-sincronização visual do rascunho no `Aplicar`). Round-trip `06 13` + `06 05` em 6 estágios validado em hardware (`~176–198 ms`).
  - **Daemon (`ajazz-daemon`)**: IPC estendido com `GetMouseStatus`, `SetMouseConfig`, `SetMouseButton`, `ResetMouseButtons`, `SyncMouseFromDevice`, `StartMousePairing`, `UploadMouseMacros`, `SetChatteringConfig` e `ResetChatteringStats` (`IpcResponse::MouseStatus` incluindo `SwitchChatteringSnapshot`); detecção automática/hot-unplug, aplicação por diff de parâmetros, drenagem de notificações assíncronas `0xFA`, refresh periódico de `06 05` (15s) e persistência em `UserProfile.mouse`.
  - **Interface Gráfica (`ajazz-gui`)**: Alinhamento concluído via **Grill Me** (3 rodadas) e aba `AJ179 NL P` implementada (`crates/ajazz-gui/ui/mouse_view.slint`, `crates/ajazz-gui/src/mouse_ui.rs`, `sidebar.slint`, `ipc_client.rs`). SVGs `shape_mouse_cima.svg` e `shape_mouse_lado.svg` adicionados na raiz para futura rodada visual (após **Grill Me**).
  - **Refinamentos de GIF Player (`9d2d67a`)**: enquadramento (`GifCrop` com modal pan/zoom) e velocidade (`AUTO` usando delay embutido + `1..30 FPS`) validados em hardware pelo usuário.

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
  - Modo `Audio` na 21ª vaga da grade `7×3` com linha contextual dos 7 estilos + toggle/cor de fundo, e barra compacta `GIF LIVE` (seletor nativo Windows `GetOpenFileNameW`, modo `AUTO` usando delay embutido do `.gif` ou passo manual `1..30 FPS`, modal `ENQUADRAR` com pan/zoom + proporção teclado `16:6`, e `PLAY/PARAR GIF` em RAM volátil `0x12` sincronizado com o Daemon).
- **Validação de Hardware Concluída:** `Audio Sync` (7 estilos + fundo) e `GIF Player (0x12)` aprovados pelo usuário no AK820 físico (paridade total do teclado concluída). Refinamento de enquadramento (`GifCrop`) e velocidade (`AUTO` + `1..30 FPS`) implementado em protocolo/daemon/GUI aguardando validação rápida em hardware.

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
- `b690faa`: `fix(gui): prevent periodic MouseStatus polls from resetting button and DPI drafts`
- `6720984`: `fix(protocol,gui): reclassify 0x40 mouse codes as ACPI System Power/Sleep/Wake`
- `f2d5833`: `fix(daemon,docs): poll mouse battery/status every 15s and record AJ179 validation`

- `9d2d67a`: `feat: add GIF crop framing and AUTO/1-30 FPS speed control`

---

## 4. Próximos Passos Imediatos

1. **Validação visual na GUI (`ajazz-gui`)**: abrir a interface para conferir proporções da logo, ícones SVG e alinhamento clicável dos 5 botões nos shapes de mouse.
2. **Versionamento**: commitar o fix validado do slot de macro `06 12` (`macro_oracle_repro.rs`) e as atualizações visuais da `ajazz-gui` (Conventional Commits).
3. **Validação restante em Hardware (`AJ179 NL P`)**: validar os modos de macro `Até soltar` e `Alternar (ON/OFF)` e a detecção ao vivo na sub-aba `DIAGNÓSTICO` (`Switch Chattering`).
