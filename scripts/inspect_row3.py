import re

with open(r"AK820_RGB_Keyboard\Skin\XML\DeviceXml\keyboarddevice.xml", "r", encoding="utf-8", errors="ignore") as f:
    c = f.read()

for m in re.finditer(r'<(\w+)[^>]*name="key_(\d+)"[^>]*>', c):
    k_num = int(m.group(2))
    tag = m.group(0)
    if k_num == 71:
        print(f"Key 71 full tag: {tag}")
