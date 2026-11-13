import math
from PIL import Image

def generate_test_gif(filename="sample_wave.gif"):
    w, h = 32, 16
    frames = []
    num_frames = 30 # ~1.2s at 25 fps

    for f in range(num_frames):
        img = Image.new("RGB", (w, h))
        pixels = []
        phase = (f / num_frames) * 2 * math.pi
        for y in range(h):
            for x in range(w):
                val = math.sin((x / w) * 4 * math.pi + phase) * math.cos((y / h) * 2 * math.pi + phase)
                hue = (f * 12 + x * 8 + y * 16) % 360
                # Simple rainbow
                r = int((math.sin(math.radians(hue)) * 0.5 + 0.5) * 255)
                g = int((math.sin(math.radians(hue + 120)) * 0.5 + 0.5) * 255)
                b = int((math.sin(math.radians(hue + 240)) * 0.5 + 0.5) * 255)
                pixels.append((r, g, b))
        img.putdata(pixels)
        frames.append(img)

    frames[0].save(
        filename,
        save_all=True,
        append_images=frames[1:],
        duration=40, # 40ms = 25 FPS
        loop=0
    )
    print(f"Salvo '{filename}' com {len(frames)} frames!")

if __name__ == "__main__":
    generate_test_gif()
