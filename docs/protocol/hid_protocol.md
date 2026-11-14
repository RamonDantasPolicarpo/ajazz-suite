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
| `0x07` | `KEY_MATRIX_MAPPING` | Configuração de mapeamento e macros das teclas (384 bytes / 7 chunks). |
| `0x0B` | `SET_CUSTOM_RGB_MATRIX` | Transmissão de buffer RGB individual para matriz de 128 LEDs (384 bytes / 7 chunks). |
| `0x1A` | `HEARTBEAT_POLL` | Pacote periódico enviado a cada ~5 segundos pelo software para manter sincronia. |
| `0x1B` | `GET_LED_MAPPING` | Leitura da tabela de roteamento dos 128 índices de LEDs físicos do hardware (128 bytes / 3 chunks). |

---

### 2.4 Anatomia Detalhada do Comando de Iluminação (`Opcode 0x06` — `sub_492b30`)

Quando `payload[3] == 0x06` (bloco de `0x1D` = 29 bytes extraído de `LightInfo` e `SystemStages` no software oficial):

| Offset | Tamanho | Campo / Valor Típico | Descrição |
| :--- | :--- | :--- | :--- |
| `0x04` | 1 byte | `0x1D` (29) | Comprimento do bloco de parâmetros (`payload[8..37]`). |
| `0x05..0x06`| 2 bytes | `00 00` | Offset LE do comando `0x06` (`0x0000`). |
| `0x07` | 1 byte | `0x00` | Reservado. |
| `0x08` | 1 byte | `0x00` | Slot de perfil do dispositivo (`arg_c = 0x00`). |
| `0x09` | 1 byte | `0x01..0x14` ou `0xFE` | **Modo de Iluminação**: `1..=18`, `20` (`0x14` Custom). Os modos `29` (`audiorecord`) e `30` (`capturecolor`) são remapeados para `0xFE` no fio. |
| `0x0A` | 1 byte | `0x00..0x04` | **Brilho (`Light`)**: `0` (apagado) a `4` (máximo). |
| `0x0B` | 1 byte | `4 - Speed` (`0x00..0x04`) | **Velocidade invertida**: o software grava `4 - Speed`, onde UI `4` (mais rápido) envia `0x00` e UI `0` (mais lento) envia `0x04`. |
| `0x0C` | 1 byte | `0x00` ou `0x01` | **Direção (`Fx`)**: `0` = L→R / Anti-horário / Cima→Baixo / Dentro→Fora; `1` = R→L / Horário / Baixo→Cima / Fora→Dentro. |
| `0x0D` | 1 byte | `0x00` ou `0x01` | **MultiColor (7-Color / Rainbow)**: `1` = ciclo 7 cores; `0` = cor sólida `(R, G, B)`. |
| `0x0E` | 1 byte | `0x00..0xFF` | **Canal Vermelho (`Red`)** da cor primária. |
| `0x0F` | 1 byte | `0x00..0xFF` | **Canal Verde (`Green`)** da cor primária. |
| `0x10` | 1 byte | `0x00..0xFF` | **Canal Azul (`Blue`)** da cor primária. |
| `0x11..0x19`| 9 bytes | `00 ... 00` | Zerado (`arg_ffffffa5..arg_ffffffad`). |
| `0x1A` | 1 byte | `0x00..0x03` | **`ZZCCLedMode`**: Sub-modo (`Group 1..4`) exclusivo do modo `17` (Surmount). |
| `0x1B` | 1 byte | `0x01` | **`SystemStages.Key6Flag`**: Padrão `1` (6-key/NKRO flag em `Keyboard.json`). |
| `0x1C` | 1 byte | `0x10` (ou `0xFF`) | **`SystemStages.Repeat`**: Padrão `16` (`0x10`) em `Keyboard.json` (observado `0xFF` em captura USB inicial). |
| `0x1D` | 1 byte | `0x00` | **`SystemStages.PollingRate`**: Padrão `0`. |
| `0x1E` | 1 byte | `0x00` | **`SystemStages.RepeatDelay`**: Padrão `0`. |
| `0x1F..0x20`| 2 bytes | `00 00` | Reservado (`0x00`). |
| `0x21` | 1 byte | `0x00..0x02` | **`CustomGroup`**: Banco ativo (`0`, `1` ou `2` -> Custom 1..3) para o Modo `0x14`. |
| `0x22` | 1 byte | `0x00` | Reservado (`0x00`). |
| `0x23` | 1 byte | `0x00` | **`SystemStages.WFlag`**: Padrão `0`. |
| `0x24` | 1 byte | `0x00` | **`SystemStages.WinFlag`**: Padrão `0` (trava tecla Win). |
| `0x25..0x3F`| 27 bytes| `00 ... 00` | Preenchimento com zeros (Padding). |

#### Tabela Completa de Modos de Iluminação (AK820 Array 0 + Capacidades de UI `sub_415010`)

| ID Decimal | Hex Wire | Nome Oficial (`lan_en.xml`) | Cor RGB | 7-Color (`MultiColor`) | Brilho | Velocidade | Direção (`Fx`) | Extras |
| :--- | :--- | :--- | :---: | :---: | :---: | :---: | :---: | :--- |
| `1` | `0x01` | **Corrugated** (`wave`) | Sim | Sim | Sim | Sim | L↔R (`0`/`1`) | — |
| `2` | `0x02` | **Cloud** (`cloud`) | Sim | Sim | Sim | Sim | L↔R (`0`/`1`) | — |
| `3` | `0x03` | **Serpentine** (`vortex`) | Sim | Sim | Sim | Sim | Clockwise / Anti (`1`/`0`) | — |
| `4` | `0x04` | **Spectrum** (`mix_color`) | Não | Fixo Multi | Sim | Sim | Não | — |
| `5` | `0x05` | **Breath** (`breath`) | Sim | Sim | Sim | Sim | Não | — |
| `6` | `0x06` | **Normal / Static** (`static`) | Sim | Sim | Sim | Não | Não | — |
| `7` | `0x07` | **Reaction** (`reaction`) | Sim | Sim | Sim | Sim | Não | — |
| `8` | `0x08` | **Ripples** (`stone`) | Sim | Sim | Sim | Sim | Não | — |
| `9` | `0x09` | **Traverse** (`traverse`) | Sim | Sim | Sim | Sim | Não | — |
| `10` | `0x0A` | **Stars** (`stars`) | Sim | Sim | Sim | Sim | Não | — |
| `11` | `0x0B` | **Flowers** (`firework_open`) | Não | Fixo Multi | Sim | Sim | Não | — |
| `12` | `0x0C` | **Roll** (`roll`) | Não | Fixo Multi | Sim | Sim | Down→Up / Up→Down (`1`/`0`) | — |
| `13` | `0x0D` | **Wave** (`wave_bar`) | Sim | Sim | Sim | Sim | Não | — |
| `14` | `0x0E` | **Cartoon** (`cartoon`) | Sim | Sim | Sim | Sim | Não | — |
| `15` | `0x0F` | **Rain** (`rain`) | Sim | Sim | Sim | Sim | Não | — |
| `16` | `0x10` | **Scan** (`scan`) | Sim | Sim | Sim | Não | Não | — |
| `17` | `0x11` | **Surmount** (`zzcc`) | Não | Fixo Multi | Sim | Sim | Não | `ZZCCLedMode` `0..=3` |
| `18` | `0x12` | **Speed** (`speed`) | Não | Fixo Multi | Sim | Não | Outside / Inside (`1`/`0`) | — |
| `20` | `0x14` | **Custom** (`custom`) | Sim (Pincel) | Não | Sim | Não | Não | `CustomGroup` `0..=2` + `0x0B` |
| `29` | `0xFE` | **Audio wave** (`audiorecord`) | Sim | Sim | Sim | Não | Não | *Oculto — pesquisa futura* |
| `30` | `0xFE` | **Capture** (`capturecolor`) | Não | Não | Sim | Não | Não | Streaming/Tela (`0xFE`) |

---

### 2.5 Sequência Transacional Obrigatória para Alteração de Cor / Modo

Para alterar o modo ou cor do teclado com sucesso, o host deve disparar exatamente a sequência de 3 pacotes:

1. **Pacote 1 (Begin Transaction):**
   ```
   04 01 00 01 00 00 00 00 00 00 00 00 ... (64 bytes)
   ```
2. **Pacote 2 (Set Lighting & Parameters):**
   ```
   04 [LSB] [MSB] 06 1D 00 00 00 00 [Modo] [Brilho] [4-Vel] [Fx] [Multi] [R] [G] [B] ... [ZZCC] [SystemStages...] [CustomGroup] ...
   ```
3. **Pacote 3 (Commit Transaction):**
   ```
   04 02 00 02 00 00 00 00 00 00 00 00 ... (64 bytes)
   ```

---

### 2.6 Matriz de Iluminação Customizada Individual (`Opcode 0x0B` — `sub_47a890`)

Para controlar a cor individual de cada tecla em modo estático por banco:

1. **Ativação de Modo**: O teclado é comutado para o **Modo 20 (`0x14` / Custom)** enviando `Opcode 0x06` com `payload[9] = 0x14` e `payload[33] = CustomGroup` (`0`, `1` ou `2` correspondendo a Custom 1..3).
2. **Buffer de Matriz e Bancos**: Cada banco possui stride de `0x200` (512 bytes) endereçando até 128 LEDs físicos × 3 canais (R, G, B) = **384 bytes úteis** (`sub_49f280` passa `offset_base = CustomGroup * 0x200`).
   - Banco 0 (Custom 1): offset base `0x0000` (`0`)
   - Banco 1 (Custom 2): offset base `0x0200` (`512`)
   - Banco 2 (Custom 3): offset base `0x0400` (`1024`)
3. **Fragmentação em Chunks de 56 Bytes**:
   O payload de cada pacote é estruturado como:
   - `payload[0] = 0x04` (Report ID)
   - `payload[1..2] = Checksum LE (soma de payload[3..64])`
   - `payload[3] = 0x0B` (Opcode)
   - `payload[4] = chunk_len` (56 para chunks 0..5, 48 para chunk 6 no total de 384 bytes)
   - `payload[5..6] = offset LE` (`(custom_group * 0x200) + chunk_offset`)
   - `payload[7] = 0x00` (Reservado)
   - `payload[8..8+chunk_len] = dados RGB brutos`

### 2.7 Limitação de Streaming e Desgaste de Memória Flash (`Opcode 0x0B`)
- O comando `Opcode 0x02` (`COMMIT_TRANSACTION`) persiste dados no setor não-volátil da memória Flash do microcontrolador.
- **Evidência Empírica no Hardware (AK820)**: Testes controlados (`flicker_diag` / `flicker_diag2`) demonstraram que **qualquer escrita contínua via `Opcode 0x0B`** (mesmo reenviando um quadro idêntico, apenas um chunk isolado ou sem `Opcode 0x02`) faz o teclado piscar visivelmente a cada escrita.
- Isso indica que o controlador EVision reprocessa a iluminação a cada pacote `0x0B` recebido, não havendo evidência de que `0x0B` opere como framebuffer volátil em RAM isolada de flash.
- **Decisão**: O streaming contínuo de GIFs por `Opcode 0x0B` é **inviável e proibido** no AK820 por causar cintilação constante e risco potencial de desgaste de memória Flash. O `Opcode 0x0B` deve ser usado exclusivamente para configuração estática pontual por tecla dentro de uma transação fechada (`0x01` -> `0x0B` -> `0x02`).
