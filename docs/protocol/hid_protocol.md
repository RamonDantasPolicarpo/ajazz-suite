# Especificação do Protocolo USB HID: Teclado AK820 e Mouse NearLink

Documentação dos identificadores de hardware, endpoints HID e mapeamento de relatórios USB obtidos por evidência direta no sistema.

---

## 1. Identificadores de Hardware (Evidência Física)

Ambos os periféricos foram enumerados e identificados diretamente através da API HID do sistema:

### 1.1 Teclado AJAZZ AK820
- **Fabricante (Manufacturer):** `Evision`
- **Produto (Product):** `AK820`
- **Vendor ID (VID):** `0x320F`
- **Product ID (PID):** `0x505B`
- **Interfaces HID Expostas:**
  - `UsagePage=0x0001, Usage=0x0006`: Teclado padrão (Boot Keyboard / HID Report de teclas)
  - `UsagePage=0x0001, Usage=0x0002`: Mouse / Ponteiro
  - `UsagePage=0x000C, Usage=0x0001`: Teclas multimídia / Controle de consumidor (Knob de volume)
  - `UsagePage=0x0001, Usage=0x0080`: Controle de sistema
  - **`UsagePage=0xFF1C, Usage=0x0092`**: **Interface Proprietária de Configuração e Iluminação**

### 1.2 Mouse AJAZZ NearLink
- **Fabricante (Manufacturer):** `AJAZZ`
- **Produto (Product):** `AJAZZ NearLink`
- **Vendor ID (VID):** `0x7768`
- **Product ID (PID):** `0x642E`
- **Interfaces HID Expostas:**
  - `UsagePage=0x0001, Usage=0x0002`: Mouse padrão (Relatórios de movimento e botões)
  - `UsagePage=0x000C, Usage=0x0001`: Atalhos de consumidor
  - **`UsagePage=0xFFB1, Usage=0x0001`**: **Interface Proprietária 1 (Telemetria/Bateria)**
  - **`UsagePage=0xFFB2, Usage=0x0001`**: **Interface Proprietária 2 (Configuração de DPI)**
  - **`UsagePage=0xFFB3, Usage=0x0001`**: **Interface Proprietária 3 (Sincronização RF/Status)**

---

## 2. Próxima Etapa: Captura de Pacotes de Iluminação (AK820)

Para mapear os bytes de `UsagePage=0xFF1C, Usage=0x0092`:
1. Mapear o `Report ID` inicial.
2. Identificar os offsets de cor RGB por tecla.
