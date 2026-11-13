# Especificação de Interface (UI Spec)

Diretrizes visuais e estruturais para a GUI do AJAZZ Unified Peripheral Suite.

---

## 1. Filosofia de Design e UX

- **Estética:** Dark mode nativo com acabamento minimalista (estilo ferramentas modernas como linear.app ou figma), sem "estética gamer de 2012" com fontes agressivas ou botões cromados.
- **Eficiência:** Janela compacta com abas de topo para alternância entre dispositivos (`AK820 Teclado` | `Mouse AJAZZ` | `Diagnóstico Global`).
- **Feedback Imediato:** Qualquer ajuste de iluminação ou DPI reflete instantaneamente em tempo real na representação virtual do dispositivo na tela.

---

## 2. Telas e Componentes Principais

### Tela 1: Visão Geral e Controle do Teclado (AK820)
- **Representação Gráfica 2D do Teclado (Layout 75%):**
  - Matriz visual de 82 teclas com LEDs renderizados individualmente.
  - Seletor de modo de iluminação:
    - Modos de Fábrica (Onda, Respiração, Estático, Reativo).
    - Modo GIF / Custom Matrix (área de drag-and-drop para arquivos `.gif`).
    - Miniatura de visualização prévia da animação sobre as teclas antes de enviar ao teclado.
  - Sliders discretos para Brilho (0-100%) e Velocidade (FPS: 15, 20, 30, 60).

### Tela 2: Monitor de Saúde dos Switches (Diagnóstico de Teclado)
- **Heatmap Interativo de Teclas:**
  - Código de cores por switch:
    - Verde: Perfeito (zero eventos de chattering).
    - Amarelo: Atenção (1-5 repetições suspeitas em < 25ms nas últimas 10.000 ativações).
    - Vermelho: Defeito crítico de switch (frequentes cliques duplos detectados).
  - Tabela de "Switches Sugeridos para Substituição": lista direta das teclas que apresentaram falhas intermitentes ao longo das sessões, com porcentagem de defeito e timestamp do último clique duplo.
  - Exportação de relatório simples de saúde dos switches.

### Tela 3: Controle e Diagnóstico do Mouse
- **Status do Dispositivo:**
  - Nível de Bateria (%) com indicador de carregamento.
  - Conexão atual (2.4GHz / USB / BT) e Polling Rate.
  - Perfil de DPI ativo com ajuste por degraus coloridos.
- **Área de Teste e Diagnóstico do Botão Esquerdo:**
  - Caixa de teste interativa ("Canvas de Teste de Clique e Arrasto"):
    - O usuário clica e arrasta para desenhar.
    - O sistema monitora se ocorrem quebras espúrias de linha durante o arrasto contínuo (detectando soltura involuntária).
    - Exibe o tempo de retenção do clique: se o botão demorar mais de $X$ segundos sem sinal de `UP`, alerta de switch travado fisicamente.
