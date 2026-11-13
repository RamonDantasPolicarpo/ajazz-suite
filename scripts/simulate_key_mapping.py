import subprocess
import struct

# 1. Read static table from executable at 0x28cf58
with open(r"AK820_RGB_Keyboard\AJAZZ AK820 有线RGB Keyboard.exe", "rb") as f:
    exe_data = f.read()

static_table = []
for k in range(126):
    off = 0x28cf58 + k * 12
    # 3 dwords
    d0, d1, d2 = struct.unpack('<III', exe_data[off:off+12])
    static_table.append((d0, d1, d2))

# 2. Extract Key Matrix from PCAP (Opcode 0x07 response, frames 35, 39, 43, 47, 51, 55, 59)
cmd = [
    r"C:\Program Files\Wireshark\tshark.exe",
    "-r", "docs/captures/ak820_red.pcapng",
    "-Y", "frame.number in {35, 39, 43, 47, 51, 55, 59}",
    "-T", "fields",
    "-e", "frame.number",
    "-e", "usbhid.data"
]
res = subprocess.run(cmd, capture_output=True, text=True)
key_matrix = bytearray()
for line in res.stdout.strip().split("\n"):
    parts = line.split("\t")
    if len(parts) == 2:
        raw = bytes.fromhex(parts[1])
        chunk_len = raw[4]
        payload = raw[8:8+chunk_len]
        key_matrix.extend(payload)

print(f"Total Key Matrix bytes: {len(key_matrix)}")

# 3. Extract LED map from PCAP (Opcode 0x1B response, frames 63, 67, 71)
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
        chunk_len = raw[4]
        payload = raw[8:8+chunk_len]
        led_map.extend(payload)

print(f"Total LED map bytes: {len(led_map)}")

# 4. Now run sub_492d60 algorithm:
# For each static key f8 in 0..126:
#   Find matching f4 in 0..num_keys where (key_matrix[f4*3..+3]) == static_table[f8]
#   If found:
#     key_to_matrix[f8] = f4
#     led_index = led_map[f4]
#     key_to_led[f8] = led_index

num_matrix_keys = len(key_matrix) // 3
key_to_matrix = {}
key_to_led = {}

for f8 in range(len(static_table)):
    target = static_table[f8]
    found_f4 = None
    for f4 in range(num_matrix_keys):
        km_entry = (key_matrix[f4*3], key_matrix[f4*3+1], key_matrix[f4*3+2])
        if km_entry == target:
            found_f4 = f4
            break
    if found_f4 is not None:
        key_to_matrix[f8] = found_f4
        key_to_led[f8] = led_map[found_f4]
    else:
        key_to_matrix[f8] = 0xFF
        key_to_led[f8] = 0xFF

print(f"Mapped {len([k for k, v in key_to_led.items() if v != 0xFF])} keys to physical LEDs!")
for k in range(25):
    print(f"Key {k:2d}: target={static_table[k]} -> matrix_idx={key_to_matrix[k]} -> phys_led={key_to_led[k]}")
