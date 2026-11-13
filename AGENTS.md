# Diretrizes do Repositório (AGENTS.md)

Este documento define as regras de engenharia, arquitetura e convenções que todo agente ou colaborador deve seguir estritamente neste repositório.

---

## 1. Princípios Arquiteturais

1. **Separação Obrigatória Daemon vs GUI**:
   - O projeto é dividido em um Cargo Workspace modular.
   - O **Daemon** (`ajazz-daemon`) deve rodar em segundo plano, headless (sem dependências de janela ou renderização), consumindo estritamente menos de 10 MB de RAM e 0% de CPU em idle.
   - A **GUI** (`ajazz-gui`) é um processo independente que só aloca memória quando a janela está aberta. Ela comunica-se com o Daemon via IPC (Named Pipe no Windows).
   - O crate de protocolo (`ajazz-protocol`) não depende de nenhuma biblioteca de UI ou IPC: apenas tipos de dados puros, serialização/desserialização binária de **HID Reports** e comunicação via `hidapi`.

2. **Engenharia Reversa Orientada a Evidências**:
   - Nunca adivinhe ou assuma estruturas de pacotes USB sem evidência capturada (USBPcap/Wireshark) ou reversa estática no Ghidra.
   - Toda estrutura de pacote deve ser documentada primeiro em `docs/protocol/` com offsets de bytes, endianness e propósito antes de ser codificada em Rust.

3. **Restrições de Desempenho e Recursos**:
   - Zero bibliotecas desnecessárias. Não traga WebViews, runtimes Node.js ou dependências pesadas de frameworks sem aprovação explícita.
   - O motor de animação de LEDs não deve alocar memória dinâmica (`Vec::new`, etc.) no loop quente de renderização de frames.

---

## 2. Regras para Geração de Código e Modelos (AGY CLI)

Para contornar limitações e comportamento prolixo de modelos como Flash e preservar cotas de Sonnet/Opus:

- **Estilo de Resposta Direto**: Seja estritamente objetivo. Sem introduções longas, sem repetições e sem rodeios.
- **Atualização Contínua de Contexto**: Sempre mantenha o arquivo `CONTEXT.md` atualizado com o status atual da fase, decisões recentes, bloqueios e próximos passos imediatos.
- **Commits Convencionais**: Todo trabalho concluído deve ser versionado usando Conventional Commits (`feat:`, `fix:`, `docs:`, `chore:`, `refactor:`).
- **Proibido Código Inflado**: Gere o mínimo de código com a máxima eficiência. Zero boilerplate desnecessário, zero abstrações prematuras ou testes fictícios.
- **Conceito Antes da Pergunta**: Se um conceito técnico novo for necessário para o usuário decidir algo, explique o conceito de forma simples e direta ANTES de fazer a pergunta.
- **Perguntas Interativas**: Sempre que for fazer perguntas de escolha/alinhamento, use o mecanismo de perguntas interativas.
- **Uso do Vocabulário Canônico**: Sempre use a terminologia definida em `GLOSSARY.md` (**Daemon**, **GUI**, **Switch Chattering**, **Key Matrix Frame**, **HID Report**, **Device Profile**).
- **Tratamento de Hardware**: Todo acesso a hardware deve prever desconexões a quente (desconectar cabo USB do teclado sem crashar o processo).
