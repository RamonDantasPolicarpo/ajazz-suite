import re

with open(r"AK820_RGB_Keyboard\AJAZZ AK820 有线RGB Keyboard.exe", "rb") as f:
    d = f.read()

for m in re.finditer(b"\x39\x15\x00\x00", d):
    idx = m.start()
    print(f"Offset {hex(idx)}: {d[max(0, idx-6):idx+10].hex(' ')}")
