#!/usr/bin/env python3
"""
Utilitário para análise e extração de pacotes HID USB do teclado AK820 a partir de arquivos PCAPNG.
"""
import json
import subprocess
import sys
from pathlib import Path

TSHARK_PATH = r"C:\Program Files\Wireshark\tshark.exe"

def parse_pcapng(pcap_path: str):
    if not Path(pcap_path).exists():
        print(f"Erro: Arquivo '{pcap_path}' não encontrado.")
        return

    cmd = [
        TSHARK_PATH,
        "-r", pcap_path,
        "-Y", 'usb.bInterfaceClass == 3',
        "-T", "json"
    ]
    res = subprocess.run(cmd, capture_output=True, text=True)
    if res.returncode != 0:
        print("Erro ao executar tshark:", res.stderr)
        return

    data = json.loads(res.stdout)
    print(f"=== Analisando '{pcap_path}' ({len(data)} frames HID encontrados) ===")

    for frame in data:
        f_num = frame["_source"]["layers"]["frame"]["frame.number"]
        t_rel = float(frame["_source"]["layers"]["frame"]["frame.time_relative"])
        usb = frame["_source"]["layers"].get("usb", {})
        src = usb.get("usb.src", "?")
        dst = usb.get("usb.dst", "?")
        setup = frame["_source"]["layers"].get("Setup Data", {})
        data_fragment = setup.get("usb.data_fragment")

        if data_fragment:
            p = bytes.fromhex(data_fragment.replace(":", ""))
            opcode = p[3]
            actual_sum = p[1] | (p[2] << 8)
            calc_sum = sum(p[3:])
            match_str = "OK" if actual_sum == calc_sum else "FAIL"
            print(f"Frame {f_num:>4s} | {t_rel:6.2f}s | {src} -> {dst} | Opcode: 0x{opcode:02X} | Checksum: 0x{actual_sum:04X} ({match_str})")
            print(f"   Payload: {p[:24].hex(' ')} ...")

if __name__ == "__main__":
    target = sys.argv[1] if len(sys.argv) > 1 else "docs/captures/ak820_red.pcapng"
    parse_pcapng(target)
