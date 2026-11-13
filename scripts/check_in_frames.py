import subprocess

cmd = [
    r"C:\Program Files\Wireshark\tshark.exe",
    "-r", "docs/captures/ak820_red.pcapng",
    "-Y", "usb.src != \"host\" and usb.data_len > 0",
    "-T", "fields",
    "-e", "frame.number",
    "-e", "frame.time_relative",
    "-e", "usb.endpoint_address",
    "-e", "usb.data_len",
    "-e", "usbhid.data"
]
res = subprocess.run(cmd, capture_output=True, text=True)
print("Keyboard IN frames:")
for line in res.stdout.strip().split("\n")[:20]:
    print(line)
