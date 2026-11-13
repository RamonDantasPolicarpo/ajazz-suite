import re

with open(r"AK820_RGB_Keyboard\Skin\XML\DeviceXml\keyboarddevice.xml", "r", encoding="utf-8", errors="ignore") as f:
    content = f.read()

# Static LED map from dump_led_map.py
from simulate_key_mapping import key_to_led

# Visible keys
tags = re.findall(r'<(\w+)\s+([^>]+)>', content)
keys = []
for tag_name, attrs in tags:
    if 'userdata=' in attrs:
        name_m = re.search(r'name="([^"]+)"', attrs)
        ud_m = re.search(r'userdata="([^"]+)"', attrs)
        pos_m = re.search(r'pos="([^"]+)"', attrs)
        if name_m and ud_m and pos_m:
            pos = pos_m.group(1)
            ud = ud_m.group(1)
            name = name_m.group(1)
            if not pos.startswith("1,1,") and ud.startswith("light_"):
                light_id = int(ud.replace("light_", ""))
                x = int(pos.split(",")[0])
                y = int(pos.split(",")[1])
                phys_led = key_to_led.get(light_id, light_id)
                keys.append((name, light_id, phys_led, x, y))

keys.sort(key=lambda k: (k[4], k[3]))
min_x = min(k[3] for k in keys) # 100
max_x = max(k[3] for k in keys) # 547
min_y = min(k[4] for k in keys) # 17
max_y = max(k[4] for k in keys) # 176

print(f"Total keys: {len(keys)}")
print(f"X: {min_x}..{max_x}, Y: {min_y}..{max_y}")

print("\nRust struct KeyPos entries:")
for name, light_id, phys_led, x, y in keys:
    nx = (x - min_x) / (max_x - min_x)
    ny = (y - min_y) / (max_y - min_y)
    print(f'    KeyPos {{ name: "{name}", light_id: {light_id}, phys_led: {phys_led}, norm_x: {nx:.4}f32, norm_y: {ny:.4}f32 }},')
