import json

# Let's inspect the instructions around 0xb1520
with open(r"AK820_RGB_Keyboard\AJAZZ AK820 有线RGB Keyboard.exe", "rb") as f:
    d = f.read()

# file offset 0xb1520 -> VA = 0xb1520 + 0x400c00 = 0x4b2120
print("Bytes around 0xb1520:")
print(d[0xb1500:0xb1600].hex(' '))
