import re

with open(r"AK820_RGB_Keyboard\Skin\XML\CustomControlXML\LightControl.xml", "r", encoding="utf-8", errors="ignore") as f:
    c = f.read()

names = re.findall(r'name="([^"]+)"', c)
print(f"Names count: {len(names)}")
for n in names:
    if any(k in n.lower() for k in ["custom", "user", "pic", "color", "btn", "led"]):
        print(" ", n)
