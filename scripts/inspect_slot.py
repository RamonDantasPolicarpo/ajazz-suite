import json

with open(r"AK820_RGB_Keyboard\DefaultData\Keyboard.json", "r", encoding="utf-8") as f:
    d = json.load(f)

for slot_idx, slot in enumerate(d["Device"][0]["CustomLightMode"]["LightColorInfo"]):
    print(f"=== Slot {slot_idx} ===")
    non_zero = []
    for i, c in enumerate(slot):
        if c["Red"] != 0 or c["Green"] != 0 or c["Blue"] != 0:
            non_zero.append((i, c["Red"], c["Green"], c["Blue"], c["Alpha"]))
    print(f"Total non-zero: {len(non_zero)}")
    for item in non_zero[:15]:
        print(f"  Key {item[0]:3d}: R={item[1]:3d}, G={item[2]:3d}, B={item[3]:3d}, A={item[4]:3d}")
