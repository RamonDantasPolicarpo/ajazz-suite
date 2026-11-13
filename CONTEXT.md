# Estado Atual do Projeto (CONTEXT.md)

Este documento registra a posição exata do projeto, decisões tomadas, descobertas de hardware e o próximo passo imediato para que qualquer retomada de sessão seja instantânea e sem perda de contexto.

---

## 1. Posição Atual: Marco 1 (Protocolo) & Marco 2 (Especificação da GUI) Concluídos

- **Engenharia Reversa USB (AK820):**
  - Canal de iluminação estática completamente decodificado via `ak820_red.pcapng`.
  - Testado e validado em tempo real via Rust nativo (`crates/ajazz-protocol/examples/set_color`).
  - Proteção estrita contra brick e desgaste de memória Flash documentada no código e no [AGENTS.md](file:///C:/Users/ramon/OneDrive/Documentos/GitHub/eng-reversa-teclado/AGENTS.md).
- **Alinhamento e Especificação da GUI (`Grill Me`):**
  - Rodada completa de 11 decisões concluída e documentada formalmente em [gui_specification.md](file:///C:/Users/ramon/OneDrive/Documentos/GitHub/eng-reversa-teclado/docs/ui/gui_specification.md).
- **Bloqueio/Aguardando:** Confirmação do usuário sobre a especificação da GUI para iniciar a implementação da arquitetura Slint / Daemon IPC.

---

## 2. Inventário de Hardware Identificado (Evidência Direta)

Detectados via ferramenta interna de enumeração (`cargo run -p ajazz-protocol --example enumerate`):

| Dispositivo | Fabricante | VID | PID | Endpoint de Controle Proprietário |
| :--- | :--- | :--- | :--- | :--- |
| **Teclado AK820** | `Evision` | `0x320F` | `0x505B` | `UsagePage=0xFF1C, Usage=0x0092` |
| **Mouse NearLink** | `AJAZZ` | `0x7768` | `0x642E` | `UsagePage=0xFFB1, 0xFFB2, 0xFFB3` |

---

## 3. Decisões de Arquitetura da Interface Gráfica (Alinhadas no Grill Me)

1. **Janela e Visual:** Slint nativo, janela compacta (~900x600 px), Dark Mode limpo (estilo Raycast/Linear).
2. **Navegação:** Sidebar vertical com seções para Teclado AK820, Mouse NearLink e Ajustes/Daemon.
3. **Ciclo de Vida:** Fechar no `X` encerra o processo gráfico da GUI e mantém ícone no System Tray para economizar RAM.
4. **Teclado:** Layout 75% vetorial interativo com preview ao vivo de cores e GIFs + destaque visual para chattering.
5. **GIF Player:** Drag & Drop direto sobre o teclado, limitado a 25 FPS para proteger o barramento USB.
6. **Mouse:** Mostrador de bateria com barra e %, seletor de estágios de DPI e painel de diagnóstico para clique preso.
7. **IPC e Persistência:** JSON em `%APPDATA%\ajazz-companion\config.json`, comunicação via `\\.\pipe\ajazz-daemon`.

---

## 4. O Que Já Foi Entregue (Histórico de Commits)

- `58779a1`: `docs: setup architectural docs, domain glossary and agent guidelines`
- `e284341`: `feat: initialize cargo workspace with protocol, daemon, and slint gui crates`
- `8f8baf2`: `feat: identify physical AK820 and NearLink mouse VID/PID and vendor usage pages`
- `e547c98`: `docs: document hardware endpoints and vendor usage pages for AK820 and NearLink mouse`
- `4248d45`: `chore: add docs captures directory for usb traffic traces`
- `5b707b3`: `feat: decode AK820 lighting protocol and implement static color control in Rust`
- `f55ddd7`: `docs: add anti-brick and flash wear-out safety rules`
- `docs/ui/gui_specification.md`: Especificação técnica e visual completa da interface gráfica.

---

## 5. Próximos Passos Imediatos

1. Confirmação do usuário sobre o documento de especificação da GUI.
2. Implementação da casca Slint (`ajazz-gui`) com a Sidebar e layout base do teclado 75%.
3. Implementação do servidor IPC Named Pipe no Daemon (`ajazz-daemon`).
