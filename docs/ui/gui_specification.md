# Especificação de Design e Arquitetura da Interface Gráfica (GUI)

Este documento formaliza todas as decisões acordadas na rodada de alinhamento técnico (*Grill Me*) para o desenvolvimento de `ajazz-gui`.

---

## 1. Diretrizes Visuais e Paradigma de Janela

- **Dimensões:** Janela compacta e moderna (~900 x 600 px), com proporções equilibradas e foco em utilidade.
- **Identidade Visual:** *Dark Mode* nativo elegante, minimalista (estética utilitária inspirada em ferramentas profissionais como Linear e Raycast), evitando saturações e clichês de estética "gamer pesada".
- **Framework de Renderização:** **Slint** (binário Rust nativo, aceleração gráfica sem Chromium/WebViews, tempo de inicialização instantâneo).
- **Ciclo de Vida:**
  - O botão de fechar (`X`) fecha a interface e desaloca todos os recursos gráficos da memória RAM.
  - A interface reside no **System Tray** (bandeja do Windows), permitindo reabertura instantânea com clique duplo ou menu de contexto.
  - O **Daemon** em segundo plano permanece rodando de forma headless (< 10 MB de RAM, 0% CPU em idle).

---

## 2. Arquitetura de Informação e Navegação

Barra lateral vertical (*Sidebar*) persistente à esquerda:

```
+---------------+------------------------------------------------------+
| [AJAZZ LOGO]  |                                                      |
|               |                                                      |
| [⌨] Teclado   |                   ÁREA DE CONTEÚDO                   |
|     AK820     |                 (Conforme aba ativa)                 |
|               |                                                      |
| [🖱] Mouse     |                                                      |
|     NearLink  |                                                      |
|               |                                                      |
| [⚙] Ajustes   |                                                      |
|     & Daemon  |                                                      |
+---------------+------------------------------------------------------+
| ● Daemon OK   | v0.1.0 (Pipe: \\.\pipe\ajazz-daemon)                 |
+---------------+------------------------------------------------------+
```

- **Status Bar Inferior:** Indicador discreto da conexão IPC com o Daemon (`Online` em verde / `Reconectando...` em amarelo).

---

## 3. Módulo 1: Teclado AJAZZ AK820

### 3.1 Layout Visual Interativo (75%)
- Renderização vetorial precisa das ~82 teclas do layout físico 75% do AK820.
- **Preview em Tempo Real:** Cada tecla exibe a cor RGB exata enviada ao teclado ou projetada quadro a quadro por uma animação/GIF.
- **Diagnóstico Integrado de Switches (Chattering):**
  - Teclas com detecção de repique/chattering recebem um contorno de destaque amarelo/alerta diretamente na tecla visual.
  - *Tooltip* ao passar o mouse exibindo número de repiques anormais detectados e tempo de bounce (em ms), identificando switches físicos com defeito para troca *hot-swap*.

### 3.2 Painel de Controle de Iluminação (Abaixo do Teclado)
- **Modos de Iluminação:** Seletor entre **Estático**, **Chuva (Rain)** e **GIF Player**.
- **Seletor de Cores:**
  - Paleta de atalhos rápidos com cores puras (Vermelho, Verde, Azul, Ciano, Amarelo, Magenta, Branco).
  - Entrada de valor Hexadecimal (`#RRGGBB`) e seletor HSV contínuo.
- **Sliders de Ajuste Fino:**
  - Brilho (Níveis 1 a 5 / 0 a 4).
  - Velocidade da animação (Níveis 1 a 5).

### 3.3 Player de Animações / GIFs
- **Importação:**
  - Suporte completo a *Drag & Drop* direto do arquivo `.gif` sobre a grade de teclas.
  - Botão tradicional de "Importar GIF" via explorador de arquivos.
- **Ajustes de Projeção:**
  - Slider de taxa de quadros (FPS): **Limitado a 25 FPS** para proteger o barramento USB e evitar instabilidades.
  - Modo de corte/redimensionamento: **Fit** (ajustar matriz mantendo proporção) ou **Crop** (preenchimento total da grade 16x6).
  - Prevenção de desgaste de Flash: O Daemon reproduz os quadros diretamente na RAM dos LEDs, sem emitir comandos de commit persistente em Flash (`0x02`).

---

## 4. Módulo 2: Mouse AJAZZ NearLink

### 4.1 Telemetria de Bateria
- Mostrador visual de nível de carga com percentual numérico exato e barra de progresso colorida (Verde > 50%, Amarelo 20-50%, Vermelho < 20%).
- Notificação/alerta discreto quando a bateria estiver abaixo de 15%.

### 4.2 Desempenho e Sensor
- **Seletores de DPI:** Estágios rápidos configuráveis (ex: 400, 800, 1600, 3200, 6400 DPI) com indicação da cor correspondente no LED do mouse.
- **Polling Rate:** Opções de 125 Hz, 500 Hz, 1000 Hz ou superior conforme suporte do protocolo NearLink.

### 4.3 Diagnóstico do Botão Esquerdo (Detecção de Clique Preso)
- Painel de telemetria em tempo real:
  - Indicador de estado do botão (Livre vs Pressionado).
  - Cronômetro de clique contínuo com alerta de clique preso (quando o sinal permanece em nível alto sem liberação física).
  - Contador de cliques totais para monitoramento de durabilidade do micro-switch.

---

## 5. Módulo 3: Configurações do Sistema e Daemon

- **Gerenciamento do Serviço Daemon:**
  - Status de latência e consumo de memória do Daemon (alvo: < 10 MB).
  - Botão de reiniciar serviço de conexão USB.
- **Inicialização Automática:**
  - Checkbox para *"Iniciar com o Windows (Minimizado na Bandeja)"*, gravando na chave de registro padrão de inicialização de usuário (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`).
- **Persistência de Dados:**
  - As preferências de iluminação, último GIF carregado e perfis de DPI são gravados em arquivo local JSON padronizado em `%APPDATA%\ajazz-companion\config.json`.

---

## 6. Comunicação IPC (Named Pipe)

- **Endereço do Pipe:** `\\.\pipe\ajazz-daemon`
- **Protocolo de Mensagens:** Mensagens codificadas em JSON delimitadas por quebra de linha (`\n`).
- **Resiliência:** A GUI tenta reconectar a cada 2 segundos caso o Daemon seja reiniciado ou finalizado, sem travar a interface e sem mensagens de erro invasivas.
