import subprocess

cmd = [
    r"C:\Program Files\Wireshark\tshark.exe",
    "-r", "docs/captures/ak820_red.pcapng",
    "-Y", "frame.number in {63, 67, 71}",
    "-T", "fields",
    "-e", "frame.number",
    "-e", "usbhid.data"
]
res = subprocess.run(cmd, capture_output=True, text=True)

led_map = bytearray()
for line in res.stdout.strip().split("\n"):
    parts = line.split("\t")
    if len(parts) == 2:
        raw = bytes.fromhex(parts[1])
        # Byte 4 is length
        chunk_len = raw[4]
        # Payload starts at byte 8
        payload = raw[8:8+chunk_len]
        led_map.extend(payload)

print(f"Total LED map bytes: {len(led_map)}")
print("LED Map:")
for i in range(0, len(led_map), 16):
    chunk = led_map[i:i+16]
    print(f"{i:3d}: " + " ".join(f"{b:02X}" for b in chunk))

# Let's correlate with visible keys from keyboarddevice.xml
import re

with open(r"AK820_RGB_Keyboard\Skin\XML\DeviceXml\keyboarddevice.xml", "r", encoding="utf-8", errors="ignore") as f:
    content = f.read()

tags = re.findall(r'<(\w+)\s+([^>]+)>', content)
visible_keys = []
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
                phys_led = led_map[light_id] if light_id < len(led_map) else 0xFF
                visible_keys.append((x, y, name, light_id, phys_led))

visible_keys.sort(key=lambda k: (k[1], k[0]))
print(f"\nVisible keys count: {len(visible_keys)}")
print("X   Y   Key Name       LightID  PhysLED")
for k in visible_keys:
    print(f"{k[0]:3d} {k[1]:3d} {k[2]:14s} {k[3]:5d}    0x{k[4]:02X} ({k[4]})")
