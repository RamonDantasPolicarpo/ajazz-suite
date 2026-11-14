# Estado Atual do Projeto (CONTEXT.md)

Este documento registra a posição exata do projeto, decisões tomadas, descobertas de hardware e o próximo passo imediato para que qualquer retomada de sessão seja instantânea e sem perda de contexto.

---

## 1. Posição Atual: Correções Críticas de Protocolo, Custom, GUI/IPC e Modo Capture (0x12) CONCLUÍDAS

- **Correções Baseadas em Evidências (`ajazz-protocol`):**
  - **Bug WASD ↔ Setas (`WFlag`) corrigido**: `sub_492b30` / `sub_44ab70` mostraram que `payload[27]` (`0x1B`) é `WFlag` (`0` normal, `1` troca WASD↔Setas) e `payload[28]` (`0x1C`) é `Key6Flag` (`0xFF`). Pacote `Opcode 0x06` agora alinha byte-a-byte com o `Frame 117` oficial de `ak820_red.pcapng`.
  - **Custom `Opcode 0x0B` + ACK HID + `RCtrl=93`**: `apply_custom_bank` agora normaliza `multicolor = false`, fecha primeiro `0x01 -> 0x06 -> 0x02` e envia os 7 pacotes `0x0B` aguardando ACK HID (`sub_4781c0`) após cada pacote; `AK820_KEY_LAYOUT[78]` (`RCtrl`, `light_id=112`) corrigido para `phys_led = 93` (`0x5D` em `sub_49ef80`).
  - **Canal Volátil `Opcode 0x12`**: Implementado `build_live_matrix_packets` / `send_live_matrix` (`378` bytes, escala `BRIGHTNESS_LUT = [0, 63, 126, 190, 255]`, zero gravações em flash `0x01`/`0x02`).
- **Daemon em Segundo Plano (`ajazz-daemon`):**
  - Captura de tela nativa leve em Windows via Win32 GDI (`screen_capture.rs`, buffer estático `32x12` zero-alocação) transmitida em ~20 FPS via `Opcode 0x12` quando o modo `Capture` (`0xFE`) está ativo, com cancelamento automático ao trocar de modo ou desconectar USB.
  - Persistência de **User Profile** em `%APPDATA%\ajazz-companion\config.json` (`profile_store.rs`).
- **Interface Gráfica Nativa (`ajazz-gui`):**
  - Canal IPC migrado de `std::sync::mpsc` bloqueante no `tokio::select!` para `tokio::sync::mpsc::unbounded_channel`, evitando travamentos de status/comandos.
  - Aplicação imediata e consistente para todos os controles visuais (modo, brilho, velocidade, direção, swatches, HSV com commit automático ao soltar/fechar popover, e Custom preservando brilho e `multicolor = false`); botão `Aplicar` mantém confirmação de digitação manual HEX/RGB.
- **Validação de Hardware Concluída:** Smoke test físico aprovado pelo usuário — WASD/Setinhas normais, modo Custom por tecla correto, resposta imediata/consistente dos controles na GUI e modo Capture (`0x12`) espelhando a tela em tempo real sem piscar.

---

## 2. Inventário de Hardware Identificado (Evidência Direta)

| Dispositivo | Fabricante | VID | PID | Endpoint de Controle Proprietário |
| :--- | :--- | :--- | :--- | :--- |
| **Teclado AK820** | `Evision` | `0x320F` | `0x505B` | `UsagePage=0xFF1C, Usage=0x0092` |
| **Mouse NearLink** | `AJAZZ` | `0x7768` | `0x642E` | `UsagePage=0xFFB1, 0xFFB2, 0xFFB3` |

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
- `[atual]`: paridade completa do subsistema de iluminação AK820 (19 efeitos + Custom 3 bancos + persistência JSON + GUI travada).

---

## 4. Próximos Passos Imediatos

1. Versionar as correções validadas (`fix:` cobrindo `WFlag`, `Custom 0x0B + ACK + RCtrl=93`, canal volátil `0x12` Capture e ergonomia GUI/IPC).
2. Reavaliar no futuro **GIF Player** e **Audio Sync (`audiorecord`)** usando o canal volátil `Opcode 0x12` (comprovadamente seguro em RAM e sem piscada, substituindo o `0x0B` antigo).
3. Escolher a próxima frente principal de paridade oficial ou novo módulo: Keymap / Macros / SystemStages (`WinLock`, `PollingRate`, `Debounce`), simulação de efeitos na GUI, ou `Mouse NearLink` / `Switch Chattering`.
