# Especificação do Protocolo USB HID: Teclado AK820 e Mouse NearLink

Documentação dos identificadores de hardware, endpoints HID e mapeamento binário de relatórios USB obtidos por engenharia reversa empírica (captura Wireshark + USBPcap).

---

## 1. Identificadores de Hardware (Evidência Física)

Ambos os periféricos foram enumerados e identificados diretamente via API HID e descritores USB:

### 1.1 Teclado AJAZZ AK820
- **Fabricante (Manufacturer):** `Evision`
- **Produto (Product):** `AK820`
- **Vendor ID (VID):** `0x320F`
- **Product ID (PID):** `0x505B`
- **Interfaces HID Expostas:**
  - Interface 0 (`UsagePage=0x0001, Usage=0x0006`): Teclado padrão (Boot Keyboard / HID Report de teclas, Endpoint `0x81 IN`)
  - Interface 1 (`UsagePage=0x0001, Usage=0x0002`): Mouse / Ponteiro
  - Interface 1 (`UsagePage=0x000C, Usage=0x0001`): Teclas de consumidor / Knob multimídia
  - Interface 1 (`UsagePage=0x0001, Usage=0x0080`): Controle de sistema
  - **Interface 1 (`UsagePage=0xFF1C, Usage=0x0092`)**: **Canal Proprietário de Controle e Iluminação LED**

### 1.2 Mouse AJAZZ NearLink
- **Fabricante (Manufacturer):** `AJAZZ`
- **Produto (Product):** `AJAZZ NearLink`
- **Vendor ID (VID):** `0x7768`
- **Product ID (PID):** `0x642E`
- **Interfaces HID Expostas:**
  - `UsagePage=0x0001, Usage=0x0002`: Mouse padrão
  - `UsagePage=0x000C, Usage=0x0001`: Atalhos de consumidor
  - `UsagePage=0xFFB1, Usage=0x0001`: Interface Proprietária 1 (Telemetria / Bateria)
  - `UsagePage=0xFFB2, Usage=0x0001`: Interface Proprietária 2 (Configuração de DPI)
  - `UsagePage=0xFFB3, Usage=0x0001`: Interface Proprietária 3 (Sincronização RF / Status)

---

## 2. Protocolo de Iluminação do Teclado AK820 (UsagePage 0xFF1C)

### 2.1 Transporte USB
- **Tipo de Transferência:** USB Control Transfer / HID `SET_REPORT` (Output Report, `0x02`)
- **bmRequestType:** `0x21` (Host-to-Device | Class | Interface)
- **bRequest:** `0x09` (`SET_REPORT`)
- **wValue:** `0x0204` (`ReportType = 0x02` Output, `ReportID = 0x04`)
- **wIndex:** `0x0001` (Interface 1)
- **wLength:** 64 bytes
- **Confirmação:** O teclado responde em loop no Endpoint de interrupção `0x82 IN` ecoando o mesmo payload de 64 bytes.

---

### 2.2 Estrutura do Pacote HID (64 Bytes)

| Offset | Tamanho | Tipo | Propósito |
| :--- | :--- | :--- | :--- |
| `0x00` | 1 byte | `u8` | **Report ID**: Sempre fixo em `0x04`. |
| `0x01` | 1 byte | `u8` | **Checksum LSB**: Byte menos significativo da soma de 16 bits de `payload[3..64]`. |
| `0x02` | 1 byte | `u8` | **Checksum MSB**: Byte mais significativo da soma de 16 bits de `payload[3..64]`. |
| `0x03` | 1 byte | `u8` | **Opcode de Comando** (ver tabela de opcodes abaixo). |
| `0x04..0x3F` | 60 bytes | bytes | **Dados do Payload / Parâmetros do Comando** (variável conforme opcode). |

#### Algoritmo do Checksum (16-bit Little-Endian Sum):
```rust
let checksum: u16 = payload[3..64].iter().map(|&b| b as u16).sum();
payload[1] = (checksum & 0xFF) as u8;        // LSB
payload[2] = ((checksum >> 8) & 0xFF) as u8; // MSB
```

---

### 2.3 Tabela de Opcodes

| Opcode | Nome | Descrição |
| :--- | :--- | :--- |
| `0x01` | `BEGIN_TRANSACTION` | Inicializa uma sessão de escrita de configuração no teclado. |
| `0x02` | `COMMIT_TRANSACTION` | Aplica e persiste a configuração enviada na memória do teclado. |
| `0x06` | `SET_LIGHTING` | Configura o modo de iluminação, velocidade, brilho e cores RGB. |
| `0x1A` | `HEARTBEAT_POLL` | Pacote periódico enviado a cada ~5 segundos pelo software para manter sincronia. |
| `0x07` | `SET_KEY_MAPPING` | Configuração de mapeamento e macros das teclas. |
| `0x1B` | `SET_CUSTOM_LIGHTING` | Bloco de matriz de iluminação customizada por tecla. |

---

### 2.4 Anatomia Detalhada do Comando de Iluminação (`Opcode 0x06`)

Quando `payload[3] == 0x06`:

| Offset | Tamanho | Valor Típico | Descrição |
| :--- | :--- | :--- | :--- |
| `0x04` | 1 byte | `0x1D` (29) | Comprimento do bloco de parâmetros (bytes `0x05` a `0x21`). |
| `0x05..0x08`| 4 bytes | `00 00 00 00` | Reservado / Índice de perfil. |
| `0x09` | 1 byte | `0x06` ou `0x0F` | **Modo de Iluminação**: `0x06` = Normal / Estático, `0x0F` = Rain (Chuva). |
| `0x0A` | 1 byte | `0x04` | **Brilho**: Nível 0 a 4 (ou 1 a 5). |
| `0x0B` | 1 byte | `0x04` | **Velocidade**: Nível de velocidade da animação. |
| `0x0C..0x0D`| 2 bytes | `00 00` | Flags de direção / animação. |
| `0x0E` | 1 byte | `0x00..0xFF` | **Canal Vermelho (Red)** da cor primária. |
| `0x0F` | 1 byte | `0x00..0xFF` | **Canal Verde (Green)** da cor primária. |
| `0x10` | 1 byte | `0x00..0xFF` | **Canal Azul (Blue)** da cor primária. |
| `0x11..0x1B`| 11 bytes| `00 ... 00` | Slots de gradiente / cores secundárias adicionais. |
| `0x1C..0x1E`| 3 bytes | `FF 00 00` | Cor de backlight / secundária. |
| `0x1F..0x20`| 2 bytes | `00 00` | Reservado. |
| `0x21` | 1 byte | `0x01` | Flag de cor sólida / ativa. |
| `0x22..0x3F`| 30 bytes| `00 ... 00` | Preenchimento com zeros (Padding). |

---

### 2.5 Sequência Transacional Obrigatória para Alteração de Cor / Modo

Para alterar o modo ou cor do teclado com sucesso, o host deve disparar exatamente a sequência de 3 pacotes:

1. **Pacote 1 (Begin Transaction):**
   ```
   04 01 00 01 00 00 00 00 00 00 00 00 ... (64 bytes)
   ```
2. **Pacote 2 (Set Lighting & Color):**
   ```
   04 [LSB] [MSB] 06 1D 00 00 00 00 [Modo] [Brilho] [Velocidade] 00 00 [R] [G] [B] ... 01 00 00 ...
   ```
3. **Pacote 3 (Commit Transaction):**
   ```
   04 02 00 02 00 00 00 00 00 00 00 00 ... (64 bytes)
   ```
