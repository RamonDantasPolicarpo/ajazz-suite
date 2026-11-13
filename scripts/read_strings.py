import struct

with open(r"AK820_RGB_Keyboard\AJAZZ AK820 有线RGB Keyboard.exe", "rb") as f:
    d = f.read()

for va in [0x675290, 0x6752b8, 0x6752dc, 0x675304, 0x675328, 0x67534c, 0x67536c]:
    rva = va - 0x400000
    foff = rva - 0x239000 + 0x238200 # rough estimate or scan
    # Let's find exact
    e_lfanew = struct.unpack('<I', d[0x3C:0x40])[0]
    opt_hdr_offset = e_lfanew + 4 + 20
    num_sections = struct.unpack('<H', d[e_lfanew+6:e_lfanew+8])[0]
    sec_hdr_offset = opt_hdr_offset + 224
    for i in range(num_sections):
        s_off = sec_hdr_offset + i * 40
        s_vsize, s_vrva, s_rawsize, s_rawoffset = struct.unpack('<IIII', d[s_off+8:s_off+24])
        if s_vrva <= rva < s_vrva + s_vsize:
            exact_foff = s_rawoffset + (rva - s_vrva)
            s = d[exact_foff:exact_foff+60].decode('utf-16le', errors='ignore').split('\x00')[0]
            print(f"{hex(va)}: '{s}'")
