import re
import sys

def main():
    path = r"AK820_RGB_Keyboard\Skin\XML\DeviceXml\keyboarddevice.xml"
    with open(path, "r", encoding="utf-8", errors="ignore") as f:
        content = f.read()

    # Find tags with userdata and name/text
    tags = re.findall(r'<(\w+)\s+([^>]+)>', content)
    print(f"Total tags: {len(tags)}")

    keys_map = []
    for tag_name, attrs in tags:
        if 'userdata=' in attrs:
            name_m = re.search(r'name="([^"]+)"', attrs)
            text_m = re.search(r'text="([^"]+)"', attrs)
            ud_m = re.search(r'userdata="([^"]+)"', attrs)
            pos_m = re.search(r'pos="([^"]+)"', attrs)
            
            name = name_m.group(1) if name_m else ""
            text = text_m.group(1) if text_m else ""
            ud = ud_m.group(1) if ud_m else ""
            pos = pos_m.group(1) if pos_m else ""
            
            if "light" in ud.lower() or "key" in name.lower():
                keys_map.append((name, text, ud, pos))

    visible_keys = []
    hidden_keys = []
    for k in keys_map:
        pos = k[3]
        if pos.startswith("1,1,"):
            hidden_keys.append(k)
        else:
            visible_keys.append(k)

    light_nums = []
    for k in keys_map:
        ud = k[2]
        if ud.startswith("light_"):
            try:
                light_nums.append(int(ud.replace("light_", "")))
            except:
                pass
    if light_nums:
        print(f"\nLight indices: Min={min(light_nums)}, Max={max(light_nums)}, Unique Count={len(set(light_nums))}")
        missing = [i for i in range(max(light_nums)+1) if i not in light_nums]
        print(f"Missing indices between 0 and {max(light_nums)}: {missing}")
    print(f"Hidden keys count: {len(hidden_keys)}")
    print("\nVisible keys:")
    for k in sorted(visible_keys, key=lambda x: (int(x[3].split(',')[1]), int(x[3].split(',')[0]))):
        print(f"Pos ({k[3]:14s}) | Name: {k[0]:12s} | UserData: {k[2]}")

if __name__ == "__main__":
    main()
