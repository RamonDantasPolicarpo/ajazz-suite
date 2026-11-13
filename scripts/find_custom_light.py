import re

with open(r"AK820_RGB_Keyboard\AJAZZ AK820 有线RGB Keyboard.exe", "rb") as f:
    d = f.read()

# Search for patterns where 0x1B is used with chunking
# In HID packets, chunk sizes were 56 (0x38), 56 (0x38), 16 (0x10)
# Total size 128 (0x80)
# Check all occurrences of 0x1B with 0x80 or 0x38 or 128
for m in re.finditer(b"\x6a\x1b", d):
    idx = m.start()
    surrounding = d[max(0, idx-60):idx+60]
    print(f"push 1Bh at {hex(idx)}: {surrounding.hex(' ')}")
