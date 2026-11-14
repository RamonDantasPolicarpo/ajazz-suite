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
| `0x01` | `BEGIN_TRANSACTION` | Inicializa uma sessão de leitura/escrita de configuração no teclado. |
| `0x02` | `COMMIT_TRANSACTION` | Aplica e persiste a configuração enviada na memória Flash não-volátil do teclado. |
| `0x03` | `GET_DEVICE_INFO` | Lê 34 bytes (`0x22`) de capacidades do firmware (`[5] = total_keys`, `[6] = blocos de 128B de macro`). |
| `0x05` | `GET_CONFIG` | Lê o perfil ativo (`[0]`) e o bloco de configuração (`0x1D` = 29 bytes) equivalente ao `0x06`. |
| `0x06` | `SET_CONFIG` / `SET_LIGHTING` | Configura o modo de iluminação, velocidade, brilho, RGB e `SystemStages` (`0x1D` = 29 bytes). |
| `0x07` | `GET_KEY_MATRIX` | **Leitura** da matriz de mapeamento de teclas atual (`total_keys × 3` bytes em chunks de até 56B). |
| `0x09` | `SET_KEY_MATRIX` | **Escrita** da matriz completa de mapeamento e vínculos de macro (`total_keys × 3` bytes, dentro de `0x01..0x02`). |
| `0x0B` | `SET_CUSTOM_RGB_MATRIX` | Transmissão de buffer RGB individual para matriz de 128 LEDs (até 384 bytes / 7 chunks). |
| `0x12` | `STREAM_VOLATILE_RGB` | Streaming volátil em RAM de 378 bytes RGB (modos Capture/Audio `0xFE`, sem tocar na Flash). |
| `0x15` | `SET_MACROS` | **Escrita** do contêiner binário `0x55AA` de macros na memória do teclado (`0x01..0x02`). |
| `0x17` | `SET_SHOOT_MACROS` | Envia macros FPS tipo *ShootData* (compensação de recuo). |
| `0x1A` | `HEARTBEAT_POLL` | Pacote periódico enviado a cada ~5 segundos pelo software para manter sincronia. |
| `0x1B` | `GET_LED_MAPPING` | Leitura da tabela `led_idx[hw_key_idx]` (`total_keys` bytes em chunks de até 56B). |

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
| `0x1B` | 1 byte | `0x00` ou `0x01` | **`SystemStages.WFlag`**: **`0` = WASD normal**, `1` = WASD ↔ Setas invertido (`sub_44ab70 +8`, `sub_492b30`). |
| `0x1C` | 1 byte | `0xFF` (ou `0x01`/`0x00`) | **`SystemStages.Key6Flag`**: `0xFF` observado no Frame 117 de `ak820_red.pcapng`; padrão `1` em `Keyboard.json` (`sub_44ab70 +4`). |
| `0x1D` | 1 byte | `0x00` ou `0x01` | **`SystemStages.WinFlag`**: Trava tecla Windows (`1` = travada, `0` = livre; `sub_44ab70 +10`). |
| `0x1E` | 1 byte | `0x00..0x03` | **`SystemStages.PollingRate`**: **`0` = 1000 Hz**, **`1` = 500 Hz**, **`2` = 250 Hz**, **`3` = 125 Hz** (`sub_49f8e0` limita `< 4`; `sub_44ab70 +6`). |
| `0x1F..0x20`| 2 bytes | `00 00` | Reservado (`0x00`). |
| `0x21` | 1 byte | `0x00..0x02` | **`CustomGroup`**: Banco ativo (`0`, `1` ou `2` -> Custom 1..3) para o Modo `0x14`. |
| `0x22` | 1 byte | `0x00` | Reservado (`0x00`). |
| `0x23` | 1 byte | `0x00` | **`SystemStages.MacMode`**: Padrão `0` (`sub_44ab70 +12`). |
| `0x24` | 1 byte | `0x00` | **`SystemStages.Wheel`**: Padrão `0` (`sub_44ab70 +14`). |
| `0x25..0x3F`| 27 bytes| `00 ... 00` | Preenchimento com zeros (Padding). |

> **Leitura (`Opcode 0x05`, `sub_47b060` + `sub_490e80`) e Configurações de Sistema no AK820**:
> - `0x05` lê 1 byte no offset `0` (`active_profile`) e `0x1D` bytes no offset `active_profile * 0x1D` com o mesmo layout de `0x06` (`payload[8..37]` ↔ resposta `[0..29]`).
> - No AK820 (`keyboarddevice.xml`), apenas **`WinFlag`**, **`WFlag`** e **`PollingRate`** são expostos; `Repeat`/`RepeatDelay` (`repeat_delay_check_`, `visible="false"`) atuam apenas no SO e **não** há ajuste de debounce por hardware no pacote `0x06`.
> - Ao atualizar apenas iluminação ou apenas `SystemStages`, o host deve preservar todos os demais campos no mesmo bloco `0x1D` para não sobrescrever `WinFlag`, `WFlag`, `PollingRate` ou a iluminação atual.

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
| `29` | `0xFE` | **Audio wave** (`audiorecord`) | Sim | Sim | Sim | Não | Não | Streaming via `Opcode 0x12` (*pesquisa futura*) |
| `30` | `0xFE` | **Capture** (`capturecolor`) | Não | Não | Sim | Não | Não | Streaming tela → teclado via **`Opcode 0x12`** (volátil) |

---

### 2.5 Sequência Transacional Obrigatória e ACK HID (`sub_4781c0`)

Para alterar o modo ou cor do teclado com sucesso, o host deve disparar a sequência de 3 pacotes lendo o **HID IN ACK (64 bytes)** após cada pacote (`resp[3] == opcode && resp[7] != 0xFF && resp[7] != 0xFE`):

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

### 2.6 Matriz de Iluminação Customizada Individual (`Opcode 0x0B` — `sub_49ef80`, `sub_49f280`, `sub_47a890`)

Para controlar a cor individual de cada tecla em modo estático por banco:

1. **Ativação de Modo**: Primeiro envia e fecha a transação `0x01 -> 0x06 -> 0x02` com `payload[9] = 0x14`, `payload[13] = 0x00` (`multicolor = false`) e `payload[33] = CustomGroup` (`0..=2`). Em seguida, envia os 7 pacotes `Opcode 0x0B` fora de `0x01..0x02`, aguardando ACK HID (`sub_4781c0`) após cada pacote.
2. **Override de Mapeamento (`sub_49ef80`)**: Para `light_id = 0x70` (`112`, `RCtrl`), o firmware usa `phys_led = 0x5D` (`93`) em vez do valor bruto da tabela (`101`).
3. **Buffer de Matriz e Bancos**: Cada banco possui stride de `0x200` (512 bytes) endereçando LEDs físicos × 3 canais (R, G, B):
   - Banco 0 (Custom 1): offset base `0x0000` (`0`)
   - Banco 1 (Custom 2): offset base `0x0200` (`512`)
   - Banco 2 (Custom 3): offset base `0x0400` (`1024`)
4. **Fragmentação em Chunks de 56 Bytes**:
   - `payload[0] = 0x04` (Report ID)
   - `payload[1..2] = Checksum LE (soma de payload[3..64])`
   - `payload[3] = 0x0B` (Opcode)
   - `payload[4] = chunk_len` (56 para chunks 0..5, 48 ou 42 no chunk 6)
   - `payload[5..6] = offset LE` (`(custom_group * 0x200) + chunk_offset`)
   - `payload[7] = 0x00` (Reservado)
   - `payload[8..8+chunk_len] = dados RGB brutos`

### 2.7 Streaming Volátil em Tempo Real (`Opcode 0x12` — `sub_47ab30` / `sub_4b4310` / `sub_492ee0`)

Canal volátil dedicado aos modos em tempo real (**Capture** `30 -> 0xFE` e **Audio** `29 -> 0xFE`), **sem** transações `0x01`/`0x02`:

1. **Ativação Inicial**: Envia uma única vez `0x01 -> 0x06(mode=0xFE, brightness=0..4) -> 0x02`.
2. **Escala de Brilho (`sub_4b4310` / `.rdata 0x682ec0`)**: O host escala cada canal via `(c * BRIGHTNESS_LUT[brightness]) >> 8` com `BRIGHTNESS_LUT = [0, 63, 126, 190, 255]`.
3. **Pacotes `Opcode 0x12`**: 7 pacotes de 64 bytes totalizando `378` bytes (`126` LEDs × 3, chunks `56×6 + 42`) com `offset LE` a partir de `0x0000` (`0, 56, ..., 336`), sem multiplicador de banco, aguardando ACK HID (`sub_478ca0`).

### 2.8 Limitação de Streaming e Desgaste de Memória Flash (`Opcode 0x0B`)
- O comando `Opcode 0x02` (`COMMIT_TRANSACTION`) persiste dados no setor não-volátil da memória Flash do microcontrolador.
- **Evidência Empírica no Hardware (AK820)**: Testes controlados (`flicker_diag` / `flicker_diag2`) demonstraram que **qualquer escrita contínua via `Opcode 0x0B`** (mesmo reenviando um quadro idêntico, apenas um chunk isolado ou sem `Opcode 0x02`) faz o teclado piscar visivelmente a cada escrita.
- **Decisão**: O streaming contínuo por `Opcode 0x0B` é **inviável e proibido** no AK820 por causar cintilação constante e risco de desgaste de memória Flash. Streaming em tempo real deve usar exclusivamente o canal volátil **`Opcode 0x12`**.

---

### 2.9 Remapeamento de Teclas (`Opcode 0x03`, `0x1B`, `0x07`, `0x09` — `sub_490cc0`, `sub_4918b0`, `sub_492d60`, `sub_410ae0`)

#### Framing Genérico de Transferência em Blocos (`sub_479630`, `sub_47a0a0`, `sub_47a340`, `sub_479910`, `sub_479bd0`)
Todos os comandos de leitura/escrita em blocos (`0x03`, `0x05`, `0x07`, `0x09`, `0x15`, `0x17`, `0x1B`) compartilham o mesmo cabeçalho de 8 bytes em relatórios OUT de 64 bytes:

| Offset | Tamanho | Valor / Descrição |
| :--- | :--- | :--- |
| `0x00` | 1 byte | `0x04` (Report ID) |
| `0x01..0x02` | 2 bytes | Checksum `u16 LE`: soma simples `payload[3..64]` |
| `0x03` | 1 byte | **Opcode** (`0x03`, `0x05`, `0x07`, `0x09`, `0x15`, `0x1B`) |
| `0x04` | 1 byte | `chunk_len` (`1..=56` bytes úteis neste chunk) |
| `0x05..0x06` | 2 bytes | `offset` `u16 LE` no buffer de destino/origem (`(profile << 9) + off` no `0x09`, `0 + off` nos demais) |
| `0x07` | 1 byte | `0x00` (Reservado) |
| `0x08..0x3F` | até 56B | Carga útil do chunk (leitura recebe os bytes em `ack[8..8+chunk_len]`; `ack[7]` ∈ `{0xFF, 0xFE}` indica erro) |

#### Descoberta de `hw_key_idx` e Estrutura da Matriz
O índice de slot na matriz `KeyMatrix` (`hw_key_idx`, `0..total_keys-1`) **não** é igual ao `phys_led` de `AK820_KEY_LAYOUT` nem ao `light_id` da UI:
1. Dentro de `0x01 -> ... -> 0x02`, `Opcode 0x03` lê `34` bytes (`[5] = total_keys`, `[6] = macro_128b_blocks`).
2. `Opcode 0x1B` (`GET_LED_MAPPING`) lê `total_keys` bytes: `led_map[hw_key_idx] = phys_led` (correspondendo diretamente ao campo `phys_led` de `AK820_KEY_LAYOUT`, com `0xFF` em slots vazios).
3. `Opcode 0x07` (`GET_KEY_MATRIX`) lê `total_keys × 3` bytes contendo registros sequenciais `[b0, b1, b2]` por `hw_key_idx`. (A tabela `.rdata 0x68dd58` contém as 126 assinaturas padrão indexadas por `light_id`.)
4. `Opcode 0x09` (`SET_KEY_MATRIX`) grava `total_keys × 3` bytes no hardware dentro da transação `0x01 -> 0x09 -> 0x02`.

#### Codificação dos Registros de Atribuição de 3 Bytes (`[b0, b1, b2]` — `sub_410ae0`, `0x6c5e20`, `0x6c5dc0`)
Cada tecla física armazena exatamente **3 bytes Big-Endian** no slot `[hw_key_idx * 3 .. hw_key_idx * 3 + 3]`:

| Padrão (`b0 b1 b2`) | Categoria | Significado exato dos bytes |
| :--- | :--- | :--- |
| `20 MM KK` | **Tecla / Combo / Desabilitar** | `MM` = bitmask de modificadores (`0x01` LCtrl, `0x02` LShift, `0x04` LAlt, `0x08` LWin, `0x10` RCtrl, `0x20` RShift, `0x40` RAlt, `0x80` RWin); `KK` = USB HID Usage (`0x04..`, `0x00` para modificador puro ou `20 00 00` para desabilitar). |
| `10 BB 00` | **Mouse** | `BB`: `0x01` Esquerdo, `0x02` Direito, `0x04` Meio, `0x08` Voltar (X1), `0x10` Avançar (X2). |
| `30 LL HH` | **Mídia / Consumer (`0x0C`)** | Usage `u16 LE`: `0x0183` Media Player, `0x00CD` Play/Pause, `0x00B7` Stop, `0x00B6` Prev, `0x00B5` Next, `0x00EA` Vol-, `0x00E9` Vol+, `0x00E2` Mute, `0x0223` Home, `0x0227` Refresh, `0x0226` WebStop, `0x0224` Back, `0x0225` Forward, `0x022A` Bookmarks, `0x0221` Search, `0x0194` MyPC, `0x0192` Calc, `0x018A` Email. |
| `40 SS 00` | **Sistema ACPI** | `SS`: `0x01` Power, `0x02` Sleep, `0x04` Wake. |
| `70 MM RR` | **Macro (Modo Contínuo/Toggle)** | `MM` = índice empacotado da macro (`0..`) na tabela `0x55AA`; `RR`: `0` = Executar ao pressionar/ciclo único (`Regular`), `1` = Parar ao soltar (`Release Stop`), `2` = Alternar liga/desliga (`Press Down` / Toggle), `3` = Parar imediatamente ao soltar. |
| `71 MM CC` | **Macro (Repetir `N` vezes)** | `MM` = índice empacotado da macro (`0..`); `CC` = quantidade de repetições (`1..=255`). |
| `72 PP 00` | **Troca de Camada/Perfil** | `PP` = índice do perfil/camada. |
| `A0 01 00` / `A0 1B 00` | **Fn / Game Mode** | `0xA0 01 00` = `Fn`; `0xA0 1B 00` = Modo Jogo (`GM`). |
| `D0 A2 NN` | **Função Especial Hardware** | `NN` = `0x01..0x05` (`M1..M5` / funções internas). |

---

### 2.10 Contêiner Binário de Macros (`Opcode 0x15` — `sub_491eb0`, `sub_479bd0`, `sub_448040`, `sub_422dd0`, `sub_42bc40`)

As macros gravadas são empacotadas em um blob contíguo e gravadas na Flash dentro da transação `0x01 -> 0x15 -> 0x02` (apenas as macros referenciadas na `KeyMatrix` ou conjunto ativo; limite máximo em bytes = `byte[6] << 7` lido de `Opcode 0x03`). Índices `MM` nos registros `0x70`/`0x71` referenciam diretamente a posição `0..macro_count-1` na tabela de offsets deste blob.

#### Cabeçalho e Tabela de Offsets (`u16` Little-Endian)
| Offset | Tamanho | Campo | Valor |
| :--- | :--- | :--- | :--- |
| `0x00..0x01` | 2 bytes | `magic` | `0x55AA` LE (`AA 55` no fio) |
| `0x02..0x03` | 2 bytes | `total_bytes` | Tamanho total do contêiner em bytes (`u16 LE`) |
| `0x04..0x05` | 2 bytes | `macro_count` | Quantidade `N` de macros no blob (`u16 LE`) |
| `0x06..0x0F` | 10 bytes | `reserved` | `00 ... 00` |
| `0x10..` | `2 × N` bytes | `offsets[0..N]` | Offset `u16 LE` em bytes a partir do início do blob (`offsets[0] = 16 + 2 * N`) |

#### Bloco de Cada Macro (`4 + 4 × event_count` bytes)
| Offset relativo | Tamanho | Campo | Descrição |
| :--- | :--- | :--- | :--- |
| `0x00..0x01` | 2 bytes | `event_count` | Número `E` de eventos desta macro (`u16 LE`) |
| `0x02..0x03` | 2 bytes | `reserved` | `00 00` |
| `0x04..` | `4 × E` bytes | `events[0..E]` | Eventos sequenciais de 4 bytes |

#### Registro de Evento de 4 Bytes (`sub_448040`, `sub_41db60`)
| Byte | Campo | Codificação |
| :--- | :--- | :--- |
| `[0..1]` | `delay_ms` (`u16 LE`) | Atraso em milissegundos (`0..=65535`). |
| `[2]` | `flags_type` (`u8`) | Bit 7 (`0x80`): `1` = Pressionar (`down`), `0` = Soltar (`up`). Bits `0..6`: `Type & 0x7F`. |
| `[3]` | `button` (`u8`) | Código da tecla/botão/delta conforme `Type`. |

Valores de `Type` observados no executável oficial:
- **`Type = 10` (`0x0A`) — Tecla HID Padrão**: `button` = USB HID Usage (`0x04..`, coluna `[4]` da tabela `0x6c5e20`; `0x8A` down / `0x0A` up).
- **`Type = 9` (`0x09`) — Tecla Modificadora**: `button` = bitmask de modificador (`0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80`, coluna `[3]` de `0x6c5e20`; `0x89` down / `0x09` up).
- **`Type = 1` (`0x01`) — Botão do Mouse**: `button` ∈ `{0x01 Left, 0x02 Right, 0x04 Middle, 0x08 Back, 0x10 Forward}` (`0x81` down / `0x01` up).
- **`Type = 4` / `5` (`0x04` / `0x05`) — Movimento Relativo Mouse X / Y**: bit 7 indica sentido (`0x84` = Esquerda `-X`, `0x04` = Direita `+X`; `0x85` = Cima `-Y`, `0x05` = Baixo `+Y`), `button` = delta `u8`.
