# 2. Proibição de Streaming Contínuo via Opcode 0x0B

Decidimos desativar o streaming contínuo de quadros (como reprodução de GIFs a 25 FPS) sobre o `Opcode 0x0B` (`SET_CUSTOM_RGB_MATRIX`), restringindo esse comando exclusivamente a configurações estáticas pontuais por tecla dentro de uma transação fechada (`0x01` -> `0x0B` -> `0x02`).

Testes controlados no hardware físico do AK820 comprovaram que toda escrita repetida via `0x0B` — mesmo reenviando quadros idênticos ou pacotes individuais sem `Opcode 0x02` — faz o controlador reprocessar a iluminação causando cintilação visível, sem garantia de que o comando opere apenas em RAM volátil livre de desgaste da memória Flash.
