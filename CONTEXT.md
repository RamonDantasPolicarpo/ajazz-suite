# Estado Atual do Projeto (CONTEXT.md)

Este documento registra a posição exata do projeto, decisões tomadas, descobertas de hardware e o próximo passo imediato para que qualquer retomada de sessão seja instantânea e sem perda de contexto.

---

## 1. Posição Atual: Marco 1 (Protocolo & Engenharia Reversa)

- **Fase:** Captura e decodificação do protocolo USB HID do teclado AK820.
- **Bloqueio/Aguardando:** Captura do pacote de cor estática (`#FF0000`) via Wireshark + USBPcap ou extração dos bytes de envio.

---

## 2. Inventário de Hardware Identificado (Evidência Direta)

Detectados via ferramenta interna de enumeração (`cargo run -p ajazz-protocol --example enumerate`):

| Dispositivo | Fabricante | VID | PID | Endpoint de Controle Proprietário |
| :--- | :--- | :--- | :--- | :--- |
| **Teclado AK820** | `Evision` | `0x320F` | `0x505B` | `UsagePage=0xFF1C, Usage=0x0092` |
| **Mouse NearLink** | `AJAZZ` | `0x7768` | `0x642E` | `UsagePage=0xFFB1, 0xFFB2, 0xFFB3` |

---

## 3. Decisões Arquiteturais Consolidadas

1. **Separação Daemon vs GUI:**
   - **Daemon (`ajazz-daemon`):** Processo headless de baixo consumo (< 10 MB RAM, 0% CPU em idle) responsável por iluminação, loop de GIFs e monitor de switches.
   - **GUI (`ajazz-gui`):** Construída com **Slint** (binário nativo sem Chromium/WebViews), aberta apenas sob demanda.
   - **Comunicação:** IPC via Named Pipes nativos do Windows (`\\.\pipe\ajazz-daemon`) com payloads em JSON.
2. **Motor de Animação de GIFs:**
   - Projeção bilinear de quadros na matriz espacial física de teclas (~82 teclas) travada em 25 FPS para não saturar o barramento USB.
3. **Monitor de Diagnóstico de Switches e Cliques:**
   - Teclado: Monitoramento passivo de switch chattering em segundo plano para identificar switches específicos para substituição física (hot-swap), sem filtros de software artificiais.
   - Mouse: Detecção contínua do estado preso do botão esquerdo correlacionada com a telemetria de nível de bateria.

---

## 4. O Que Já Foi Entregue (Histórico de Commits)

- `58779a1`: `docs: setup architectural docs, domain glossary and agent guidelines`
- `e284341`: `feat: initialize cargo workspace with protocol, daemon, and slint gui crates`
- `8f8baf2`: `feat: identify physical AK820 and NearLink mouse VID/PID and vendor usage pages`
- `e547c98`: `docs: document hardware endpoints and vendor usage pages for AK820 and NearLink mouse`
- `4248d45`: `chore: add docs captures directory for usb traffic traces`

---

## 5. Próximos Passos Imediatos

1. Receber o arquivo de captura ou os bytes hexadecimais do pacote de cor estática.
2. Documentar a anatomia dos 64 bytes em `docs/protocol/hid_protocol.md` (identificando Report ID, opcode de cor, offsets R/G/B e checksum).
3. Implementar a rotina `set_static_color(color: RgbColor)` em `crates/ajazz-protocol`.
4. Testar a troca de cor ao vivo a partir do binário Rust compilado.
