# Estado Atual do Projeto (CONTEXT.md)

Este documento registra a posição exata do projeto, decisões tomadas, descobertas de hardware e o próximo passo imediato para que qualquer retomada de sessão seja instantânea e sem perda de contexto.

---

## 1. Posição Atual: Marco 1 (Protocolo & Engenharia Reversa) - Concluído para Cor Estática

- **Fase:** Engenharia reversa concluída com sucesso para o canal de iluminação estática e modos globais do AK820.
- **Evidência Decodificada (`docs/captures/ak820_red.pcapng`):**
  - **Interface:** `UsagePage=0xFF1C, Usage=0x0092`, Interface 1, Report ID `0x04`.
  - **Checksum:** 16-bit Little-Endian Sum de `payload[3..64]`.
  - **Transação:** Sequência de 3 pacotes: `Begin (0x01)` -> `Set Lighting (0x06)` -> `Commit (0x02)`.
  - **Status:** Testado e validado em tempo real com o binário Rust `set_color` alterando a cor do teclado para vermelho e azul.
- **Bloqueio/Aguardando:** Nenhum bloqueio para cor estática.

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
- `docs/protocol/hid_protocol.md`: Documentação matemática e estrutural completa do protocolo de iluminação e checksum do AK820.
- `crates/ajazz-protocol`: Implementação da API de controle de iluminação (`Ak820Device`, `calculate_checksum`, construtores de pacotes transacionais, testes de unidade).
- `examples/set_color`: Utilitário CLI para troca de cores em tempo real via Rust nativo.

---

## 5. Próximos Passos Imediatos

1. Mapeamento da matriz de LEDs individuais por tecla (Opcode `0x1B` ou pacote de Custom Lighting para projeção de GIFs).
2. Construção do servidor IPC no Daemon (`ajazz-daemon`) via Named Pipe do Windows.
3. Integração inicial da UI Slint com o Daemon para controle de iluminação.
