# Estado Atual do Projeto (CONTEXT.md)

Este documento registra a posição exata do projeto, decisões tomadas, descobertas de hardware e o próximo passo imediato para que qualquer retomada de sessão seja instantânea e sem perda de contexto.

---

## 1. Posição Atual: Marco 1 (Protocolo), Marco 2 (Daemon IPC), Marco 3 (GUI Slint) e Mapeamento de Matriz (Estático) CONCLUÍDOS

- **Engenharia Reversa USB (AK820):**
  - Protocolo de iluminação estática (`0x06`) e modo Rain (`0x0F`) validados fisicamente.
  - Mapeamento espacial das 82 teclas físicas do AK820 e `Opcode 0x0B` decodificados e documentados em [hid_protocol.md](file:///C:/Users/ramon/OneDrive/Documentos/GitHub/eng-reversa-teclado/docs/protocol/hid_protocol.md).
  - **Limitação Descoberta no Hardware**: Testes empíricos confirmaram que toda escrita contínua via `Opcode 0x0B` faz o MCU reprocessar a iluminação causando piscada visível. Por ausência de prova de que `0x0B` opere em RAM sem risco de flash wear-out, o streaming contínuo (GIF Player) foi desativado no Daemon e removido da GUI.
  - Regras de anti-brick e flash wear-out registradas em [AGENTS.md](file:///C:/Users/ramon/OneDrive/Documentos/GitHub/eng-reversa-teclado/AGENTS.md).
- **Daemon em Segundo Plano (`ajazz-daemon`):**
  - Servidor IPC Named Pipe (`\\.\pipe\ajazz-daemon`) ativo para controle de cor estática, modos nativos e matriz pontual.
  - `PlayGif` desativado (`GIF_STREAMING_ENABLED = false`) com retorno de erro explicativo.
  - Consumo em repouso: **< 10 MB de RAM** e **0% CPU em idle**.
- **Interface Gráfica Nativa (`ajazz-gui`):**
  - Construída em **Slint**, compilada em binário nativo executável (`target/debug/ajazz-gui.exe`).
  - **Design Monocromático & Tipografia:** Paleta grafite escuro fosco (`#18181B`), linhas sutis de 1px (`#32323A`), tipografia JetBrains Mono.
  - **Layout do AK820 75%:** Grade física interativa com ~82 teclas e knob rotativo no topo direito.
  - **Controles de Iluminação:** Modos Estático e Chuva (Rain), paleta rápida de cores puras + botão "Aplicar Cor".
  - **Abas Integradas:** Sidebar vertical com abas para Teclado AK820, Mouse NearLink e Configurações/Daemon.
- **Bloqueio/Aguardando:** Nenhum bloqueio. Alinhamento completo (`/grill-with-docs`) concluído para paridade do módulo de iluminação do software oficial.

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
- `[próximo]`: `chore: disable gif streaming due to hardware flicker and remove gif tab from gui`

---

## 4. Próximos Passos Imediatos

1. Engenharia reversa estática dos payloads completos de `Opcode 0x06` (modos `1..18` e `30`, flag Multicolor, brilho, velocidade, direção, sub-variações) e teste de transação única (`0x01 -> 0x0B -> 0x02`) para o Modo 20 Custom.
2. Implementar paridade completa do módulo de iluminação no `ajazz-protocol`, `ajazz-daemon` (com persistência de **User Profile** em JSON) e `ajazz-gui` (janela travada, 19 efeitos + Custom 3 bancos).
3. Engenharia reversa das features de entrada/sistema (Keymap, Macros, Win Lock, Polling Rate), Mouse NearLink e Switch Chattering.
