# Protocolo HID Proprietário — Mouse AJAZZ AJ179 NL P (`NearLink BS25/BS2X`)

Documentação orientada a evidências obtida por engenharia reversa estática do binário oficial (`AJAZZ_Mouse/AJAZZ Driver（W1）.exe`, SHA-256 `bdb57ab194f4e399882331da8a83104fbac8bc6ea270a42ce4a6555593b808df`) e arquivos de configuração (`AJAZZ_Mouse/config.xml`, `AJAZZ_Mouse/device/aj179.xml`, `AJAZZ_Mouse/language/1033.lan`).

> [!CAUTION]
> **Política Anti-Brick e Proibição de DFU/Firmware** ([ADR-0003](../adr/0003-mouse-anti-brick-and-dfu-lockout.md)):
> - **Nunca** acessar a interface `UsagePage 0xFFB1 / Usage 0x01`, nem executar rotinas de `BurnTool.exe`, `libburn.dll`, `AJAZZ_Mouse/firmware/`, modo J-Link (`BS25-Jlink`, `BS2X-Jlink`) ou `DFU_FAIL_DEL_REG.bat`.
> - O comando `06 0F 01 00 00 FF` (Factory Reset, `sub_431ce0`) é documentado apenas para referência e **não deve ser exposto em automações**.
> - Todo acesso HID ao mouse deve restringir-se estritamente a `UsagePage == 0xFFB2` e `Usage == 0x01`.

---

## 1. Identificação do Dispositivo e Transporte HID

| Modo de Conexão | VID | PID | Interface USB | Modelo XML |
| :--- | :--- | :--- | :--- | :--- |
| **Dongle 2.4GHz NearLink** | `0x7768` | `0x642E` | `MI_01` | `device_type=103 (0x67)`, `device_attr="aj179"` |
| **Cabo USB (Wired)** | `0x7768` | `0x6D2E` | `MI_01` | `button="5"` (`201..205`), `light="0"` (sem RGB) |

- **Filtro de Enumeração (`sub_449710`)**:
  - `UsagePage == 0xFFB2`
  - `Usage == 0x01`
  - `OutputReportByteLength == 0x40` (`64` bytes)
- **Camada de Transporte Windows (`sub_44acd0`, `sub_44adb0`, `sub_449260`)**:
  - **Escrita (`WriteFile`)**: envia buffers de **64 bytes** completos com padding de `0x00` nas posições não utilizadas. Byte `0` é `0x06` (Report ID proprietário / prefixo do comando).
  - **Leitura (`ReadFile`)**: se o primeiro byte retornado pelo HID for `0x00` (Report ID `0` implícito), `sub_44adb0` descarta esse byte inicial e copia até 64 bytes a partir do deslocamento `+1`; caso contrário, copia a partir de `+0`.
  - **Multibyte**: todos os inteiros de 16 bits neste protocolo são **Little-Endian (`u16_le`)**, salvo indicação explícita.

---

## 2. Comandos de Leitura e Status

### 2.1 `06 05` — Ler Status e Perfil Ativo (`sub_42f930`, `ev_839bdd...`)

- **TX (64 bytes)**: `06 05 00 00 ...` (o software oficial tenta até 5 vezes com intervalo de 20 ms se `rx[9] == 0 && rx[10] == 0`).
- **RX (64 bytes pós-normalização de Report ID `0x00`)**:

| Offset | Tamanho | Campo | Codificação / Valores |
| :--- | :--- | :--- | :--- |
| `5..6` | `u16_le` | Estado interno 1 | `[edi+0x7cf]` |
| `7..8` | `u16_le` | Estado interno 2 | `[edi+0x7d1]` |
| `9` | `u8` | Mouse FW Patch | Inteiro decimal (`%d`) |
| `10` | `u8` | Mouse FW Major.Minor | Byte hex (`%02x`), ex.: `0x12` -> `"1.2.<patch>"` |
| `11` | `u8` | Modo de conexão | `[edi+0x7d7]` (`3` vs outros) |
| `12` | `u8` | Online / Acordado | `0` = offline / em repouso; `!= 0` = ativo |
| `13` | `u8` | Bateria (%) | `0..100` (validado em hardware: `0x32` = 50%, `[edi+0x7da]`) |
| `14` | `u8` | Carregando | `0` = desconectado do cabo; `1` = carregando (`[edi+0x7d9]`) |
| `16` | `u8` | `dpi_count` | Estágios ativos (`1..6`) |
| `17` | `u8` | `dpi_index` | Índice do estágio selecionado (`0..5`) |
| `18..29` | `6 × u16_le` | Estágios DPI 1..6 | `DPI = (raw_u16_le + 1) * 50` (`50..26000` DPI em passos de `50`) |
| `30` | `u8` | `report_rate` | `0 = 125 Hz`, `1 = 250 Hz`, `2 = 500 Hz`, `3 = 1000 Hz` |
| `31` | `u8` | `report_flag` | `[edi+0x7de]` |
| `32` | `u8` | `scroll_flag` | `0 = Forward` (normal), `1 = Reverse` (invertido) |
| `33` | `u8` | `liftoff_height` | `0 = 1.0 mm`, `1 = 2.0 mm` |
| `34` | `u8` | `sensor_flag` (bitmask) | `bit 0 (0x01)` = Ripple Control, `bit 1 (0x02)` = Linear Correction, `bit 2 (0x04)` = Motion Sync |
| `35` | `u8` | `highspeed_mode` | `0 = Off`, `1 = On` (no cabo USB, a UI força `1` e desabilita a troca) |
| `36` | `u8` | `button_respondtime` | Debounce dos botões: `1..50` ms (`0x01..0x32`) |
| `37..38` | `u16_le` | `light_sleep` | Tempo de sleep leve |
| `39..40` | `u16_le` | `deep_sleep` | Tempo de deep sleep |

### 2.2 `06 07` — Ler Versão do Receptor 2.4GHz (`sub_42f930`)

- **TX (64 bytes)**: `06 07 00 00 ...`
- **RX**: quando `rx[5] == 1`, a versão do dongle é formatada a partir de `rx[10]` (`%02x` -> `major.minor`) e `rx[9]` (`patch`, `%d`).

### 2.3 `06 10` e `06 11` — Sondagens de Inicialização (`sub_42f930`)

- **TX**: `06 10 00 ...` e `06 11 00 ...` (`expect_reply = 1`, payload de resposta ignorado pelo driver).

---

## 3. Comandos de Escrita de Parâmetros (`06 13`)

Todos os comandos da família `0x13` compartilham um cabeçalho fixo de 6 bytes (`pkt[0..5]`) com o payload a partir do byte `6` (restante dos 64 bytes preenchido com `0x00`):

```
[0]=0x06  [1]=0x13  [2]=<len>  [3]=0x00  [4]=0x00  [5]=<param_id>  [6..]=<payload>
```

| `param_id` | `len` | Função | Payload (`pkt[6..]`) | Limites / Evidência |
| :--- | :--- | :--- | :--- | :--- |
| `0x01` | `0x05` | Definir DPI e estágio | `[6]=dpi_count`, `[7]=dpi_index`, `[8..9]=u16_le(dpi_value)` | `sub_4312b0` (`ev_275e3e...`). Faixa AJ179: `50..26000` (passo `50`), `1..6` estágios. *(Nota: disassembly mostra `_wtoi` direto para `dpi_value` na escrita enquanto `06 05` lê `(raw+1)*50`; validar em hardware!)* |
| `0x02` | `0x02` | Polling Rate | `[6]=report_rate` (`0..3`) | `sub_431490` (`ev_06b9df...`). `0=125Hz, 1=250Hz, 2=500Hz, 3=1000Hz`. |
| `0x05` | `0x04` | Direção do Scroll | `[6]=scroll_flag` (`0`/`1`), `[7..9]=00 00 00` | `sub_431790` (`ev_71a96f...`). `0=Forward`, `1=Reverse`. |
| `0x06` | `0x02` | Distância de Lift-off (LOD) | `[6]=liftoff_height` (`0`/`1`) | `sub_431890` (`ev_73c98d...`). `0=1.0mm`, `1=2.0mm`. |
| `0x07` | `0x02` | Flags do Sensor | `[6]=sensor_flag` (bitmask `0x00..0x07`) | `sub_431590` (`ev_9af2f8...`). `bit0=Ripple`, `bit1=Linear`, `bit2=MotionSync`. |
| `0x08` | `0x02` | High-Speed Mode | `[6]=highspeed_mode` (`0`/`1`) | `sub_431990` (`ev_6bc864...`). |
| `0x09` | `0x01` | Iniciar Pareamento NearLink | `[6]=0x00` | `sub_431be0` (`ev_4a26f4...`) / `sub_443580`. |
| `0x0A` | `0x02` | Debounce (`button_respondtime`) | `[6]=delay_ms` (`1..50`) | `sub_431690` (`ev_3fa0bf...`). |
| `0x0B` | `0x05` | Temporizadores de Sleep | `[6..7]=light_sleep:u16_le`, `[8..9]=deep_sleep:u16_le` | `sub_431a90` (`ev_e954d6...`). |

---

## 4. Remapeamento de Botões (`06 12`, `sub_4310e0`, `sub_415790`)

O pacote de 64 bytes usa cabeçalho de 5 bytes e slots consecutivos de 4 bytes iniciando em `pkt[5 + slot_idx * 4]` (`ev_0162ab...`, `ev_1ea7c5...`):

- **Cabeçalho (`pkt[0..4]`)**: `06 12 20 00 00`
- **Slots padrão inicializados em `sub_4310e0`**:
  - `slot 0` (`pkt[5..8]`): `10 01 00 00` (Botão Esquerdo / Key 1)
  - `slot 1` (`pkt[9..12]`): `10 02 00 00` (Botão Direito / Key 2)
  - `slot 2` (`pkt[13..16]`): `10 04 00 00` (Botão Meio / Key 3)
  - `slot 3` (`pkt[17..20]`): `10 10 00 00` (Avançar / Key 4)
  - `slot 4` (`pkt[21..24]`): `10 08 00 00` (Voltar / Key 5)
  - `slot 5` (`pkt[25..28]`): `10 20 00 00`
  - `slot 6` (`pkt[29..32]`): `10 40 00 00`
  - `slot 7` (`pkt[33..36]`): `1F 01 00 00` (DPI+)
  - `slot 8` (`pkt[37..40]`): `1F 00 00 00` (DPI-)
- **Codificação por Tipo de Ação (`[b0, b1, b2, b3]`, `sub_415790`)**:
  - **Desabilitado (`8`)**: `00 00 00 00`
  - **Botão de Mouse (`1`)**:
    - Esquerdo=`10 01 00 00`, Direito=`10 02 00 00`, Meio=`10 04 00 00`, Duplo Clique=`10 01 02 00`
    - Avançar=`10 10 00 00`, Voltar=`10 08 00 00`, Extra6=`10 20 00 00`, Extra7=`10 40 00 00`
    - Scroll Up=`10 00 00 FE`, Scroll Down=`10 00 00 02`, DPI+=`1F 01 00 00`, DPI-=`1F 00 00 00`
  - **Teclado (`2`)**: `20 <mod_bits> <k1> <k2>` (se tecla modificadora única `0xE0..0xE7` sem `mod_bits`, promove bit para `b1` e zera `b2`)
  - **Mídia / Consumer (`4`)**: `30 <usage_u16_le> 00` (ex.: `0x00CD` Play/Pause, `0x00B7` Stop, `0x00B6` Prev, `0x00B5` Next, `0x00E9` Vol+, `0x00EA` Vol-, `0x00E2` Mute, `0x0183` Player, `0x0192` Calc, `0x018A` Mail)
  - **Atalho / Sistema ACPI (`7`, `sub_4247b0` + `sub_415a80`)**:
    - Subcategorias Windows/Escritório: `20 <mod_bits> <hid_key> 00` ou Media `30 <usage_u16_le> 00`
    - Subcategoria `System management class` (`0x1b`/`0x1c`/`0x1d`): **`40 <01|02|04> 00 00`** = **Sistema ACPI (`SystemKey`: `01=Power`, `02=Sleep`, `04=Wake Up`, HID `UsagePage 0x01 / Usage 0x80`)** — **não é Sniper DPI**.
    - Nota sobre *Sniper key*: `215=Sniper key` / `216=Set Sniper key` (`0x54`) em `sub_435ea0` não têm handler em `sub_4209c0` nem opcode no firmware AJ179; troca de DPI em botão usa `DPI+` (`1F 01 00 00`) ou `DPI-` (`1F 00 00 00`).
  - **Especial (`9, 10, 11`)**: `50 <09|0A|0B> <val> 00`
  - **Macro (`5`)**: `60 <macro_idx> <loop_mode> <loop_count>` (`loop_mode`: `0=N vezes`, `1=até soltar`, `2=até pressionar novamente`)

---

## 5. Upload de Macros (`06 31`, `sub_430860`, `ev_16d19d...`)

- **Contêiner**: buffer de até `0xFA0` (`4000` bytes).
  - `[0x00..0x3F]`: tabela de 32 offsets `u16_le` (inicializada em `0x0040`), indicando o início do corpo de cada macro.
  - `[0x40..]`: sequência de eventos de **4 bytes** `[delay_lo, delay_hi, tipo_flags, codigo]`:
    - `0x42` / `0x02`: Tecla HID normal Press / Release (`0x41` / `0x01` para modificadores)
    - `0x43` / `0x03`: Botão de mouse Press / Release (`codigo` = máscara)
    - `0x44`: Scroll (`0x01` = cima, `0xFF` = baixo)
    - `0x06`: Movimento XY ou código especial
    - **Terminador obrigatório após o último evento**: `[final_delay:u16_le, 0x83, 0x01]`.
- **Transporte em Fatias (`06 31`)**:
  - Até `59` bytes (`0x3B`) por pacote HID, intervalo de 10 ms entre pacotes:
  - `[0]=0x06, [1]=0x31, [2]=<chunk_len (<=0x3B)>, [3..4]=<offset:u16_le>, [5..63]=<data>`

---

## 6. Outros Comandos e Notificações Assíncronas

- **Apply / Refresh (`sub_431c40`, `ev_7ea2ac...`)**: `06 0F 01 00 00 08` (seguido de zeros).
- **Troca de Perfil (`sub_431250`, `ev_05ea8e...`)**: `52 52 <profile_id> 00` (`expect_reply = 0`).
- **Notificações Assíncronas IN (`sub_434ae0`, `ev_5ea767...`)**:
  - Identificadas quando `rx[1] == 0xFA`; o tipo de evento está em `rx[5]`:
    - `0x01` ou `0x06`: Mouse reconectado / acordou (re-consultar estado com `06 05`)
    - `0x02`: Estágio de DPI alterado fisicamente no mouse (`rx[6] = dpi_index`)
    - `0x03`: Atualização de bateria (`rx[6] = battery %`, `rx[7] = charging`)
    - `0x50`: Evento de pareamento / ação (`rx[6]`, `rx[7]`)
