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
O defeito de contato mecânico/elétrico em que um switch registra múltiplos acionamentos elétricos espúrios em um intervalo temporal anormalmente curto.
_Avoid_: Double click, tecla fantasma, repetição acidental.

**Key Matrix Frame**:
A representação bidimensional espacial de cores RGB mapeada diretamente sobre as coordenadas físicas das teclas do periférico para exibição de animações e efeitos.
_Avoid_: GIF frame, tela de LED, array de cores.

**HID Report**:
A estrutura de dados binária padronizada de tamanho fixo transmitida via barramento USB entre o sistema operacional e o microcontrolador do periférico.
_Avoid_: Pacote, mensagem, payload bruto.

**Device Profile**:
A definição estática das capacidades de um periférico suportado, incluindo Vendor ID (VID), Product ID (PID), mapa de comandos HID e layout físico.
_Avoid_: Configuração, modelo, device spec.
