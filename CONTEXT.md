# Estado Atual do Projeto (CONTEXT.md)

Este documento registra a posição exata do projeto, decisões tomadas, descobertas de hardware e o próximo passo imediato para que qualquer retomada de sessão seja instantânea e sem perda de contexto.

---

## 1. Posição Atual: Paridade de SystemStages (0x05/0x06), Remapeamento (0x03/0x1B/0x07/0x09) e Macros (0x15) VALIDADA EM HARDWARE

- **Protocolo e Engenharia Reversa (`docs/protocol/hid_protocol.md` + `ajazz-protocol`):**
  - **`SystemStages` (`Opcode 0x05` / `0x06`)**: `HID[27]=WFlag`, `HID[28]=Key6Flag`, `HID[29]=WinFlag`, `HID[30]=PollingRate` (`0..3` -> `1000/500/250/125 Hz`), `HID[35]=MacMode`, `HID[36]=Wheel`. Configurado de forma unificada (`build_config_packet` / `parse_config_block`) para preservar iluminação e flags de sistema juntos.
  - **Keymap (`Opcode 0x03`, `0x1B`, `0x07`, `0x09`)**: Descoberta dinâmica de `total_keys` e blocos de macro (`0x03`), mapa `led_map[hw_key_idx] == phys_led` (`0x1B`), leitura base da `KeyMatrix` (`0x07`), registros de 3 bytes Big-Endian (`Tecla/Combo`, `Mouse`, `Mídia/Consumer`, `Sistema ACPI`, `Macro 0x70/0x71`, `Camada`, `Fn`, `Desabilitar`) com `AK820_FACTORY_ASSIGNMENTS` (82 teclas) e escrita fechada em transação (`0x01 -> 0x09 -> 0x02`).
  - **Macros (`Opcode 0x15`)**: Contêiner binário `0x55AA` LE empacotado com tabela de offsets e eventos de 4 bytes (`Type 10` HID, `9` Modificador, `1` Botão Mouse, `4/5` Movimento X/Y relativo).
- **Daemon (`ajazz-daemon`):**
  - `UserProfile` estendido com `system`, `keymap` e `macros` retrocompatíveis (`#[serde(default)]`).
  - Novos comandos IPC (`SetSystemStages`, `SetKeyAssignment`, `ResetKeymap`, `UpsertMacro`, `DeleteMacro`, `SyncFromDevice`), mantendo proteção contra hot-unplug e preservando `SystemStages` em todas as chamadas de iluminação.
- **Interface Gráfica Nativa (`ajazz-gui`):**
  - Sub-abas em `AK820 RGB` (`Iluminação`, `Remapear Teclas`, `Macros`, `Sistema`) mantendo o teclado 82 teclas fixo no topo (`980x680` travado) com destaque de seleção e ponto indicador de remapeamento.
  - Editor de Keymap por categorias + captura de tecla física (`FocusScope`), Editor/Gravador híbrido de Macros ao vivo + manual e sub-aba Sistema com alerta visual para `WFlag` (`WASD ↔ Setas`) e botão `Sincronizar do Teclado (0x05)`.
- **Validação de Hardware Concluída:** Smoke test físico aprovado pelo usuário no AK820 — `WinLock`, `WASD ↔ Setas`, `Polling Rate`, remapeamento de teclas (`0x09`), gravação/vinculação de macros (`0x15`) e `Sincronizar do Teclado (0x05)` funcionando corretamente.

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
- `3c5ed87`: paridade completa do subsistema de iluminação AK820 (19 efeitos + Custom 3 bancos + Capture `0x12` + persistência JSON + GUI travada).
- `ab78f2f`: `feat: implement AK820 SystemStages, keymap remapping and macro editor` (paridade completa do teclado AK820: iluminação + sistema + keymap + macros validados no hardware).

---

## 4. Próximos Passos Imediatos

1. Reavaliar e reativar **GIF Player** e implementar **Audio Sync (`audiorecord`)** usando o canal volátil `Opcode 0x12` (RAM segura, sem piscada nem desgaste de Flash).
2. Adicionar simulação animada em tempo real dos efeitos RGB na própria **GUI** (`KeyboardView`).
3. Escolher a próxima frente de expansão: engenharia reversa do **Mouse NearLink** (`0x7768:0x642E`) ou detector de **Switch Chattering** + filtros de diagnóstico.
