# Estado Atual do Projeto (CONTEXT.md)

Este documento registra a posição exata do projeto, decisões tomadas, descobertas de hardware e o próximo passo imediato para que qualquer retomada de sessão seja instantânea e sem perda de contexto.

---

## 1. Posição Atual: Paridade Completa do Subsistema de Iluminação AK820 IMPLEMENTADA

- **Engenharia Reversa Estática e Protocolo HID (`ajazz-protocol`):**
  - Anatomia completa dos 29 bytes (`0x1D`) de `Opcode 0x06` (`sub_492b30`, `LightInfo`, `SystemStages`) e matriz de capacidades dos 19 Lighting Effects (`sub_415010`) decodificada e documentada em [hid_protocol.md](file:///C:/Users/ramon/OneDrive/Documentos/GitHub/eng-reversa-teclado/docs/protocol/hid_protocol.md).
  - Descobertas críticas corrigidas no fio: inversão `4 - Speed` no byte `0x0B`, flag `MultiColor` (7-Color) no byte `0x0D`, direção `Fx` no byte `0x0C`, `ZZCCLedMode` (`0..3`) no byte `0x1A`, `CustomGroup` (`0..2`) no byte `0x21`, remapeamento `0xFE` para `Capture`/`Audio`, e stride `0x200` (512 bytes) por banco Custom em `Opcode 0x0B` (`sub_47a890`).
  - Validação física executada em hardware (`Rain MultiColor`, `Corrugated` L↔R, `Custom` banco 0 com transação única fechada `0x01 -> 0x06 -> 0x0B -> 0x02` e restauração `Static`).
- **Daemon em Segundo Plano (`ajazz-daemon`):**
  - Persistência de **User Profile** em `%APPDATA%\ajazz-companion\config.json` (`profile_store.rs`) para estado de iluminação e 3 bancos Custom estáticos (`Custom 1..3`).
  - Canal IPC Named Pipe atualizado para `LightingConfig`, `SetCustomBank` e `SelectCustomBank` com sincronização inicial de perfil ao cliente GUI.
  - `PlayGif` mantido desativado (`GIF_STREAMING_ENABLED = false`) para pesquisa futura segura de modo Áudio/streaming.
- **Interface Gráfica Nativa (`ajazz-gui`):**
  - Janela travada em `980x680` (`min = max`), alinhamento simétrico e tema grafite fosco (`#18181B`) em `JetBrains Mono`.
  - Seletor completo dos 19 efeitos nativos de firmware + Modo 20 `Custom`, controles dinâmicos de Brilho (`0..4`), Velocidade (`0..4`), Direção (`Fx`), `7-Color`, `Surmount Preset (1..4)`, 3 bancos Custom com pintura/borracha por tecla, `#HEX`, `R/G/B`, swatches rápidos e seletor HSV contínuo em popover sob demanda.
- **Bloqueio/Aguardando:** Validação visual final pelo usuário na GUI executável; pesquisa futura para streaming seguro de Audio wave (`29`) isolada em standby.

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

1. Validação interativa pelo usuário do comportamento dos 19 efeitos e dos 3 bancos Custom na GUI (`ajazz-daemon` + `ajazz-gui`).
2. Decidir próxima etapa de paridade oficial: Keymap / Macros / SystemStages (`WinLock`, `PollingRate`) ou módulos dedicados (`Mouse NearLink` / `Switch Chattering`).
3. Investigar em momento oportuno protocolo de áudio em tempo real do modo `audiorecord` (`29`) para entender se há canal volátil seguro diferente de `0x0B`.
