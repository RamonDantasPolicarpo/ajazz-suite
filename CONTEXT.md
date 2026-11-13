# Estado Atual do Projeto (CONTEXT.md)

Este documento registra a posição exata do projeto, decisões tomadas, descobertas de hardware e o próximo passo imediato para que qualquer retomada de sessão seja instantânea e sem perda de contexto.

---

## 1. Posição Atual: Marco 1 (Protocolo) & Marco 2 (Daemon IPC) Concluídos com Sucesso

- **Engenharia Reversa USB (AK820):**
  - Canal de iluminação estática completamente decodificado via `ak820_red.pcapng`.
  - Proteção estrita contra brick e desgaste de memória Flash documentada no código e no [AGENTS.md](file:///C:/Users/ramon/OneDrive/Documentos/GitHub/eng-reversa-teclado/AGENTS.md).
- **Daemon Headless (`ajazz-daemon`):**
  - Servidor IPC nativo via Named Pipe do Windows (`\\.\pipe\ajazz-daemon`) implementado com Tokio assíncrono.
  - Padrão **Actor** (`device_manager.rs`) com posse exclusiva do hardware USB e tolerância a desconexões a quente (*hot-unplug*).
  - Protocolo de mensagens JSON com comandos: `ping`, `get_status`, `set_static_color`, `set_lighting`.
  - **Métricas de Hardware Reais Validadas:** **7.37 MB de RAM** (meta: < 10 MB) e **0% de CPU em idle**.
  - Testado e validado em tempo real via cliente IPC: comandos `green` e `purple` aplicados no teclado físico instantaneamente.
- **Alinhamento da GUI (`Grill Me`):**
  - Especificação completa em [gui_specification.md](file:///C:/Users/ramon/OneDrive/Documentos/GitHub/eng-reversa-teclado/docs/ui/gui_specification.md) com tema grafite escuro fosco (`#18181B`), tipografia JetBrains Mono e representação das teclas em borda sólida nítida (sem degradês/neon).
- **Bloqueio/Aguardando:** Nenhum bloqueio. Prontos para construir a GUI Slint integrada diretamente ao Named Pipe.

---

## 2. Inventário de Hardware Identificado (Evidência Direta)

| Dispositivo | Fabricante | VID | PID | Endpoint de Controle Proprietário |
| :--- | :--- | :--- | :--- | :--- |
| **Teclado AK820** | `Evision` | `0x320F` | `0x505B` | `UsagePage=0xFF1C, Usage=0x0092` |
| **Mouse NearLink** | `AJAZZ` | `0x7768` | `0x642E` | `UsagePage=0xFFB1, 0xFFB2, 0xFFB3` |

---

## 3. Decisões Arquiteturais Consolidadas

1. **Separação Daemon vs GUI:**
   - **Daemon (`ajazz-daemon`):** 7.37 MB RAM, 0% CPU em idle. Dono do acesso USB. Escuta em `\\.\pipe\ajazz-daemon`.
   - **GUI (`ajazz-gui`):** Processo Slint independente, aberta sob demanda, fecha no `X` para o System Tray liberando 100% da RAM da UI.
2. **Motor de Animação de GIFs:**
   - Projeção bilinear travada a 25 FPS para proteger o barramento USB, sem commits na Flash do chip (reprodução em RAM).
3. **Monitor de Diagnóstico de Switches:**
   - Detecção passiva de repiques no Daemon reportada via IPC para destaque visual na GUI.

---

## 4. O Que Já Foi Entregue (Histórico de Commits)

- `58779a1`: `docs: setup architectural docs, domain glossary and agent guidelines`
- `e284341`: `feat: initialize cargo workspace with protocol, daemon, and slint gui crates`
- `8f8baf2`: `feat: identify physical AK820 and NearLink mouse VID/PID and vendor usage pages`
- `e547c98`: `docs: document hardware endpoints and vendor usage pages for AK820 and NearLink mouse`
- `4248d45`: `chore: add docs captures directory for usb traffic traces`
- `5b707b3`: `feat: decode AK820 lighting protocol and implement static color control in Rust`
- `f55ddd7`: `docs: add anti-brick and flash wear-out safety rules`
- `034df4c`: `docs: document comprehensive GUI architecture and design specification from grill-me session`
- `1ddf438`: `docs: adjust GUI palette to graphite-neutral dark tone and JetBrains typography`
- `crates/ajazz-daemon`: Servidor IPC Named Pipe (`\\.\pipe\ajazz-daemon`) com actor de hardware, 7.37 MB RAM e 0% CPU.

---

## 5. Próximos Passos Imediatos

1. Construção da interface gráfica em Slint (`ajazz-gui`):
   - `theme.slint` (tokens grafite e JetBrains Mono).
   - Componente de tecla `keycap.slint` com borda sólida.
   - Grade interativa 75% e Sidebar de navegação.
2. Conexão do cliente IPC na GUI para acionamento direto dos comandos de cor no Daemon.
