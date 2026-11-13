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

- **RF-KB01 - Controle de Iluminação Estática e Predefinida:**
  - Ajuste de cores RGB por tecla ou global.
  - Seleção de brilho, velocidade e direção de efeitos de fábrica.
- **RF-KB02 - Motor de Animação de GIFs (Custom Frame Engine):**
  - Carregamento de arquivos `.gif` pelo usuário na GUI.
  - Decodificação de frames e mapeamento bilinear/projeção na matriz física de teclas (~82 teclas + knob).
  - Controle de taxa de atualização de frames (15–30 FPS configurável) com interpolação de cores.
- **RF-KB03 - Monitor Passivo de Switch Chattering (Identificação para Troca):**
  - Monitoramento contínuo em segundo plano durante o uso real (jogos, digitação).
  - Registro de ocorrências intermitentes de clique duplo (delta entre soltura e clique $< 25-30\text{ ms}$).
  - Contabilizador persistente por switch físico para indicar com precisão quais switches devem ser trocados (hot-swap).
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
