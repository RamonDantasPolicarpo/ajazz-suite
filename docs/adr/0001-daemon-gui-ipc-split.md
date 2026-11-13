# 1. Separação Arquitetural entre Daemon e GUI via IPC

Decidimos estruturar o software como um Cargo Workspace desacoplado em duas partes principais: um **Daemon** headless que roda continuamente em segundo plano e uma **GUI** que abre apenas sob demanda do usuário, comunicando-se via Named Pipe local.

Fizemos essa escolha para garantir que o monitoramento contínuo de hardware, telemetria e reprodução de efeitos RGB na matriz consumam menos de 10 MB de RAM e 0% de CPU em idle, sem carregar contextos gráficos pesados ou bibliotecas de janelas permanentemente na memória do sistema operacional.
