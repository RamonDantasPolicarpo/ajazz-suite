import json

with open(r"C:\Users\ramon\.gemini\antigravity-cli\brain\28bd2fff-33e2-4950-9695-95ea829894ea\.system_generated\steps\511\output.txt", "r", encoding="utf-8") as f:
    line = f.readline()
    d = json.loads(line)

code = d["result"]["pseudocode"]
print(f"Total pseudocode length: {len(code)} characters")
lines = code.split("\n")
print(f"Total lines: {len(lines)}")

# Let's inspect where HidD_SetOutputReport is called and what's passed to it
for i, line in enumerate(lines):
    if "HidD_SetOutputReport" in line or "0x1b" in line.lower() or "0x07" in line.lower() or "checksum" in line.lower():
        print(f"Line {i:3d}: {line}")
