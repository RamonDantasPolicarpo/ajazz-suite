# 3. Bloqueio Anti-Brick e Proibição de DFU/Firmware no Mouse AJ179 NL P

**Status:** Obrigatório / Permanente (`AJAZZ_Mouse`, NearLink `BS25` / `BS2X`)

## Contexto

O diretório do software oficial do mouse **AJ179 NL P** (`VID 0x7768`, `PID 0x642E` em 2.4G / `PID 0x6D2E` em USB) inclui ferramentas de gravação de firmware (`AJAZZ_Mouse/BurnTool/`, `libburn.dll`, `upgrade.bat`), imagens binárias (`AJAZZ_Mouse/firmware/`) e scripts de recuperação DFU (`DFU_FAIL_DEL_REG.bat`).

## Decisão

1. **Bloqueio Total de Firmware / Bootloader / DFU**:
   - É estritamente proibido executar, automatizar, invocar ou implementar rotinas baseadas em `BurnTool.exe`, `libburn.dll`, `AJAZZ_Mouse/firmware/`, modo J-Link (`BS25-Jlink`, `BS2X-Jlink`), `config_dfu`, bootloader ou comandos HID de entrada em DFU/IAP/ISP.
2. **Proibição de Fuzzing ou Bytes Arbitrários**:
   - Nenhum comando HID proprietário (`UsagePage 0xFFB1` / `0xFFB2` / `0xFFB3` ou `MI_01`) pode ser enviado ao mouse sem evidência estática prévia (descompilação de `AJAZZ Driver（W1）.exe`) e/ou captura USB verificada (Wireshark/USBPcap).
3. **Proteção de Memória Não-Volátil (Flash/EEPROM)**:
   - Gravações de perfil, DPI, polling rate, LOD, debounce ou macros devem ocorrer apenas sob ação explícita do usuário, nunca em loop contínuo.
