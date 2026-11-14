# Especificação de Design e Arquitetura da Interface Gráfica (GUI)

Este documento formaliza todas as decisões acordadas na rodada de alinhamento técnico (*Grill Me*) para o desenvolvimento de `ajazz-gui`.

---

## 1. Diretrizes Visuais e Paradigma de Janela

- **Dimensões:** Janela compacta com **tamanho travado** (`min` = `max`, alvo inicial `980 x 680 px`, sem permitir redimensionamento ou maximização deformante e sem scroll).
- **Identidade Visual:** *Dark Mode* nativo elegante, minimalista (estética utilitária em grafite fosco `#18181B`, sem degradês/neon nos LEDs virtuais, usando apenas bordas sólidas nítidas).
- **Framework de Renderização:** **Slint** (binário Rust nativo, aceleração gráfica sem Chromium/WebViews, tempo de inicialização instantâneo).
- **Tipografia:** `JetBrains Mono` embutida, garantindo alinhamento tabular preciso de valores (RGB, `#HEX`, DPI, Bateria %, ms de debounce).
- **Ciclo de Vida:**
  - Fechar no botão (`X`) encerra diretamente o processo da **GUI**, liberando 100% de RAM/GPU gráfica. *(System Tray adiado para após consolidação das funcionalidades principais.)*
  - O **Daemon** em segundo plano permanece rodando de forma headless (< 10 MB de RAM, 0% CPU em idle).

### 1.1 Tokens de Cores Monocromáticas (Grafite Neutro)

A interface utiliza uma paleta grafite fosca (evitando preto puro absoluto), com contraste equilibrado e isolamento estrito de cor:

| Elemento | Hex | Propósito Visual |
| :--- | :--- | :--- |
| **Background Base** | `#18181B` | Fundo principal da janela (grafite escuro fosco, ~10% de luminância). |
| **Superfícies / Cards** | `#222227` | Painéis elevados, sidebar e base do teclado. |
| **Bordas / Linhas** | `#32323A` | Divisórias e contornos em *hairline* fina (1px). |
| **Hover / Destaque** | `#2B2B33` | Estado ativo/hover de botões, abas e sliders. |
| **Texto Primário** | `#F4F4F5` | Títulos, valores ativos e dados tabulares. |
| **Texto Secundário** | `#A1A1AA` | Descrições e rótulos de controle. |
| **Texto Mudo / Labels** | `#71717A` | Unidades (`ms`, `FPS`, `DPI`), atalhos e placeholders. |
| **Isolamento de Cor** | Funcional | A interface em si é 100% monocromática. A cor só existe nos LEDs físicos das teclas e indicadores críticos. |

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
- **Preview Estático e Custom:** Nos **19 Lighting Effects** nativos do firmware, a grade exibe indicação visual estática (bordas sólidas na cor selecionada ou padrão indicativo neutro quando *Multicolor* estiver ativo, mantendo 0% de sobrecarga de GPU). No **Modo 20 (Custom)**, cada uma das ~82 teclas exibe sua cor individual configurada e responde ao clique para pintar ou apagar. *(Simulação animada de efeitos na GUI poderá ser avaliada apenas após consolidação completa das features.)*
- **Diagnóstico Integrado de Switches (Switch Chattering — fase futura):**
  - Teclas com detecção de **Switch Chattering** recebem um contorno de destaque amarelo/alerta diretamente na tecla visual com *tooltip* detalhado.

### 3.2 Painel de Controle de Iluminação (Abaixo do Teclado)
- **Seletor de Lighting Effect:** Seleção entre todos os **19 modos nativos do firmware** (`1..18` e `30`) mais o **Modo 20 (Custom Per-Key Estático)**.
- **Parâmetros Dinâmicos do Efeito:**
  - Toggle **Multicolor / 7 Cores** vs **Cor Sólida** (conforme suporte de cada modo).
  - Controles discretos de **Brilho (`0..4`)** e **Velocidade (`0..4`)**.
  - Seletor de **Direção** (Esquerda / Direita) para os modos direcionais (`Wave`, `Traverse`, `Roll`, `Wave bar`, `Scan`) e sub-variação (`1..4`) para o modo `Surmount` (`17`).
- **Seletor de Cores:**
  - Paleta de atalhos rápidos com cores puras + inputs numéricos inline (`R`, `G`, `B` e `#HEX`).
  - Seletor HSV contínuo aberto sob demanda em *popover* ao clicar na amostra de cor atual, evitando poluir o painel compacto.

### 3.3 Modo Custom Estático por Tecla (Modo 20 / 3 Bancos)
- Alternância entre os **3 bancos salváveis** (*Custom 1, 2 e 3*).
- Ferramenta de **Pintar Tecla** (aplica a cor selecionada na tecla clicada) e **Apagar Tecla** (`#000000` / LED desligado).
- Envio pontual por transação fechada (`0x01 -> 0x0B -> 0x02`), sem streaming contínuo, conforme [ADR-0002](file:///C:/Users/ramon/OneDrive/Documentos/GitHub/eng-reversa-teclado/docs/adr/0002-no-continuous-0x0b-streaming.md). *(A antiga seção de GIF Player a 25 FPS está suspensa enquanto aguardamos investigação do modo `29` Áudio.)*

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
- **Persistência de Dados (`User Profile`):**
  - As preferências do **User Profile** ativo (último **Lighting Effect**, cor, brilho, velocidade, direção, multicolor, os 3 bancos Custom e futuras configurações de mouse) são gravadas localmente em `%APPDATA%\ajazz-companion\config.json` para manter **Daemon** e **GUI** sincronizados ao reabrir.

---

## 6. Comunicação IPC (Named Pipe)

- **Endereço do Pipe:** `\\.\pipe\ajazz-daemon`
- **Protocolo de Mensagens:** Mensagens codificadas em JSON delimitadas por quebra de linha (`\n`).
- **Resiliência:** A GUI tenta reconectar a cada 2 segundos caso o Daemon seja reiniciado ou finalizado, sem travar a interface e sem mensagens de erro invasivas.
