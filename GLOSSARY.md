# AJAZZ Unified Peripheral Suite

Contexto de domínio para o conjunto unificado de controle de hardware, telemetria e diagnóstico para teclado e mouse AJAZZ em Rust.

## Language

**Daemon**:
O processo em segundo plano responsável por manter a comunicação contínua via USB HID, executar rotinas de animação e capturar métricas dos dispositivos sem interface gráfica.
_Avoid_: Driver, background worker, service, backend.

**GUI**:
A aplicação gráfica interativa executada sob demanda pelo usuário para configuração, personalização visual e exibição de diagnósticos.
_Avoid_: Frontend, painel, client app.

**Switch Chattering**:
O defeito de contato mecânico/elétrico em que um switch (de tecla ou botão) registra múltiplos acionamentos elétricos espúrios em um intervalo temporal anormalmente curto.
_Avoid_: Double click, tecla fantasma, repetição acidental.

**Stuck Input**:
A condição em que o sistema operacional mantém uma entrada logicamente pressionada após a soltura física do switch, geralmente causada pela perda do HID Report de liberação ou travamento mecânico do contato.
_Avoid_: Botão preso, clique travado, tecla agarrada.

**Key Matrix Frame**:
A representação bidimensional espacial de cores RGB mapeada diretamente sobre as coordenadas físicas das teclas do periférico para exibição de animações e efeitos.
_Avoid_: GIF frame, tela de LED, array de cores.

**HID Report**:
A estrutura de dados binária padronizada de tamanho fixo transmitida via barramento USB entre o sistema operacional e o microcontrolador do periférico.
_Avoid_: Pacote, mensagem, payload bruto.

**Device Profile**:
A definição estática das capacidades físicas de um periférico suportado, incluindo Vendor ID (VID), Product ID (PID), mapa de comandos HID e layout físico.
_Avoid_: Configuração, modelo, device spec, perfil de usuário.

**User Profile**:
Um conjunto nomeado e persistente de preferências configuradas pelo usuário, incluindo o efeito de iluminação ativo, mapeamento de teclas e macros associados.
_Avoid_: Device profile, preset, configuração global.

**Lighting Effect**:
Um modo de iluminação executado nativamente pelo microcontrolador do periférico com seus parâmetros configuráveis (cor, modo multicolorido, brilho, velocidade e direção).
_Avoid_: Animação, efeito RGB, tema, modo de LED.

**Keymap**:
A tabela de atribuição lógica entre cada posição física de tecla (incluindo a camada secundária `Fn`) e a função ou código de entrada emitido pelo periférico.
_Avoid_: Layout, bind, tecla customizada.

**Macro**:
Uma sequência cronológica gravada ou editada de pressionamentos, liberações e atrasos temporais atribuída a um acionamento de tecla ou botão.
_Avoid_: Script, atalho, combo.
