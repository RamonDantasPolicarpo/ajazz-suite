# Requisitos do Sistema: AJAZZ Unified Peripheral Suite

Documento vivo de requisitos funcionais e não funcionais para o ecossistema de periféricos AJAZZ em Rust.

---

## 1. Escopo e Objetivos

Substituir o software proprietário fragmentado e pesado por uma solução única, leve e confiável em Rust para periféricos AJAZZ (iniciando pelo teclado AJAZZ AK820 e expandindo para o mouse da mesma marca).

---

## 2. Requisitos Não Funcionais (RNF)

- **RNF01 - Pegada de Recursos (Daemon):** O Daemon em segundo plano não deve ultrapassar 10 MB de consumo de memória RAM e deve manter 0% de uso de CPU quando nenhum efeito dinâmico estiver em execução.
- **RNF02 - Desacoplamento:** A GUI só deve consumir recursos de GPU/RAM enquanto estiver visível. Fechar a janela encerra o processo da GUI ou suspende sua renderização sem interromper as animações do teclado.
- **RNF03 - Tolerância a Falhas:** Desconexões a quente do cabo USB ou trocas de modo sem fio não devem causar pânico (`panic!`) no processo. O Daemon deve tentar reconexão automática com backoff exponencial.
- **RNF04 - Latência de Entrada:** O monitor de switches não deve introduzir latência perceptível (hook assíncrono ou buffer de eventos de alta precisão).

---

## 3. Requisitos Funcionais: Teclado AJAZZ AK820 (RF-KB)

- **RF-KB01 - Controle Completo de Iluminação Nativa e Customizada (Prioridade Atual):**
  - Suporte aos **19 Lighting Effects** executados nativamente no firmware do AK820 (`1..18` e `30`), com controle completo dos parâmetros do **HID Report**: cor RGB primária (`#HEX` / RGB / HSV), flag Multicolor / 7 Cores, Brilho (`0..4`), Velocidade (`0..4`), Direção (Esquerda/Direita) e sub-variações (ex: modo Surmount `17`).
  - Suporte ao **Modo 20 (`0x14` / Custom Per-Key Estático)** com **3 bancos** salváveis (*Custom 1, 2 e 3*), permitindo acender ou apagar teclas individuais em uma única transação fechada (`0x01 -> 0x0B -> 0x02`).
  - Persistência do **User Profile** ativo (último **Lighting Effect**, parâmetros e bancos Custom) em `%APPDATA%\ajazz-companion\config.json`, sincronizando **Daemon** e **GUI** na inicialização.
- **RF-KB02 - Streaming Contínuo de Key Matrix Frame / Modo Áudio (Suspenso):**
  - *Status*: Suspenso conforme [ADR-0002](file:///C:/Users/ramon/OneDrive/Documentos/GitHub/eng-reversa-teclado/docs/adr/0002-no-continuous-0x0b-streaming.md). Streaming contínuo de GIFs via `Opcode 0x0B` fica desativado (`GIF_STREAMING_ENABLED = false`) por causar cintilação e risco de desgaste de Flash; investigação futura dependerá de engenharia reversa do modo `29` (*Audio Wave* / `AudioRecord.exe`).
- **RF-KB03 - Recursos de Entrada e Sistema do Software Oficial (Fase Posterior à Iluminação):**
  - **Keymap**: Remapeamento das teclas padrão, multimídia, mouse, atalhos com modificadores e disparo de **Macros** (sem editor de camada `Fn`, pois o firmware mantém `Fn` fixa).
  - **Macro**: Gravação/edição de sequências de teclas, movimentos/botões de mouse e atrasos em ms.
  - Configurações globais: Win Lock, troca WASD ↔ Setas, 6KRO vs NKRO, Polling Rate (125–1000 Hz) e Repeat Rate/Delay.
- **RF-KB04 - Monitor Passivo de Switch Chattering (Identificação para Troca):**
  - Monitoramento contínuo em segundo plano pelo **Daemon** durante o uso real (jogos, digitação).
  - Registro de ocorrências intermitentes de acionamento espúrio (delta entre soltura e clique $< 25-30\text{ ms}$).
  - Contabilizador persistente por switch físico para indicar com precisão na **GUI** quais switches devem ser trocados (hot-swap).
  - Sem filtros de bloqueio artificial no teclado: monitoramento puro para diagnóstico de hardware.

---

## 4. Requisitos Funcionais: Mouse AJAZZ (RF-MS)

- **RF-MS01 - Telemetria Básica:**
  - Consulta periódica de nível de bateria (%).
  - Identificação de DPI ativo e taxa de polling (125/250/500/1000 Hz).
  - Status de conexão (Cabo USB vs Dongle 2.4GHz vs Bluetooth).
- **RF-MS02 - Diagnóstico do Botão Esquerdo Travado:**
  - Monitorar eventos de `WM_LBUTTONDOWN` sem o respectivo `WM_LBUTTONUP` (estado "preso").
  - Correlacionar a ocorrência do travamento com o nível de bateria atual e perda de pacotes de rádio para isolar a causa:
    - Hipótese A: Queda de tensão/bateria fraca gerando perda do pacote de liberação.
    - Hipótese B: Desgaste físico na mola do microswitch.

---

## 5. Requisitos de Interface (RF-UI)

Ver especificação detalhada em [ui_spec.md](file:///C:/Users/ramon/OneDrive/Documentos/GitHub/eng-reversa-teclado/docs/ui_spec.md).
