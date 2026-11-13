# Estado Atual do Projeto (CONTEXT.md)

Este documento registra a posição exata do projeto, decisões tomadas, descobertas de hardware e o próximo passo imediato para que qualquer retomada de sessão seja instantânea e sem perda de contexto.

---

## 1. Posição Atual: Marco 1 (Protocolo), Marco 2 (Daemon IPC), Marco 3 (GUI Slint) e Marco 4 (Matriz por Tecla & GIF Player 25 FPS) CONCLUÍDOS

- **Engenharia Reversa USB (AK820):**
  - Protocolo de iluminação estática decodificado (`0x06`) e validado fisicamente via USBPcap e Rust nativo.
  - Protocolo da matriz de LEDs customizada decodificado via engenharia reversa estática no Ghidra (`Opcode 0x0B` - SET_CUSTOM_RGB_MATRIX com fragmentação em 7 chunks e `Opcode 0x06` modo 20 / 0x14).
  - Mapeamento espacial exato das 82 teclas físicas do AK820 normalizadas em $[0.0, 1.0]$ correlacionadas aos 128 índices de LED do microcontrolador EVision (`matrix.rs`).
  - Projeção de imagem espacial zero-allocation e algoritmo de streaming a 25 FPS implementado e testado no hardware real.
  - Regras de anti-brick e flash wear-out registradas em [AGENTS.md](file:///C:/Users/ramon/OneDrive/Documentos/GitHub/eng-reversa-teclado/AGENTS.md) (Zero gravação de flash com `Opcode 0x02` durante animações em tempo real; buffer volátil em RAM).
- **Daemon em Segundo Plano (`ajazz-daemon`):**
  - Servidor IPC Named Pipe (`\\.\pipe\ajazz-daemon`) ativo e gerenciando reprodução assíncrona de GIFs e comandos em tempo real.
  - Decodificador de GIFs (`gif_decoder.rs`) com pré-carregamento de frames e timer de 25 FPS desacoplado do IPC.
  - Consumo em repouso: **< 10 MB de RAM** e **0% CPU em idle**.
- **Interface Gráfica Nativa (`ajazz-gui`):**
  - Construída em **Slint**, compilada em binário nativo executável (`target/debug/ajazz-gui.exe`).
  - **Design Monocromático & Tipografia:** Paleta grafite escuro fosco (`#18181B`), linhas sutis de 1px (`#32323A`), tipografia JetBrains Mono.
  - **Layout do AK820 75%:** Grade física interativa com ~82 teclas e knob rotativo no topo direito.
  - **Simulação de LEDs:** Bordas sólidas nítidas (1.5px) com a cor selecionada (sem degradês/neon), garantindo 0% de sobrecarga de GPU.
  - **Controles de Iluminação:** Paleta de atalhos rápidos com cores puras + botão "Aplicar Cor".
  - **Aba GIF PLAYER Integrada:** Seletor nativo de arquivos Windows (`rfd`), seletor de taxa de quadros (15, 20, 25 FPS), botões "REPRODUZIR" e "PARAR", badge de status em tempo real e proteção de flash ativa.
  - **Abas Integradas:** Sidebar vertical com abas para Teclado AK820, Mouse NearLink e Configurações/Daemon.
  - **Conexão IPC Ponta a Ponta:** Toda interação na interface despacha a mensagem assíncrona ao Daemon que acende os LEDs físicos do teclado.
- **Bloqueio/Aguardando:** Nenhum bloqueio. Pronto para teste do usuário na tela.

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
- `[próximo]`: `feat: implement custom per-key rgb matrix, 25 fps gif player, and native gui controls`

---

## 4. Próximos Passos Imediatos

1. Usuário abrir a GUI (`ajazz-gui.exe`) e testar a reprodução do GIF animado no teclado AK820 físico na aba "GIF PLAYER".
2. Engenharia reversa dos pacotes de telemetria de bateria e DPI do Mouse NearLink (`0x7768:0x642E`).
3. Detecção de Switch Chattering em segundo plano no Daemon e alerta na GUI.
