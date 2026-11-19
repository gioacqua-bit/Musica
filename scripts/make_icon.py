"""Render the Musica app icon: an Aizawa-attractor particle cloud on a dark rounded square.
Writes the icon set Tauri expects into src-tauri/icons/. Requires numpy and Pillow."""
import numpy as np
from PIL import Image, ImageDraw, ImageFilter

def F(x):
    X, Y, Z = x; zb = Z - 0.7
    return np.stack([zb*X - 3.5*Y, 3.5*X + zb*Y, 0.6 + 0.95*Z - Z**3/3 - (X*X + Y*Y)*(1 + 0.165*Z) + 0.155*Z*X**3])
def rk4(x, h):
    k1 = F(x); k2 = F(x + h/2*k1); k3 = F(x + h/2*k2); k4 = F(x + h*k3)
    return x + h/6*(k1 + 2*k2 + 2*k3 + k4)

rng = np.random.default_rng(3); n = 20000
x = np.array([[0.1], [0], [0]]) + rng.random((3, n)) - 0.5
for _ in range(2500): x = rk4(x, 0.01)
pts, spd = [], []
for _ in range(60):
    for _ in range(4): x = rk4(x, 0.01)
    pts.append(x.copy()); spd.append(np.linalg.norm(F(x), axis=0))
P = np.concatenate(pts, 1); S = np.concatenate(spd)

R = 1024; pitch, yaw, roll = 0.32, 0.4, -0.18
c, s = np.cos(yaw), np.sin(yaw)
X = c*P[0] - s*P[1]; Yd = s*P[0] + c*P[1]; Z = P[2] - 0.65
u = X; v = np.sin(pitch)*Yd + np.cos(pitch)*Z
u, v = np.cos(roll)*u - np.sin(roll)*v, np.sin(roll)*u + np.cos(roll)*v
sc = R*0.205
px = (R/2 + u*sc).astype(int); py = (R/2 - v*sc).astype(int)
ok = (px >= 0) & (px < R) & (py >= 0) & (py < R)
t = np.clip(0.5 + 0.5*np.log(np.maximum(S, 1e-6)/1.17)/np.log(5.0/1.17), 0, 1)
t = np.clip(0.5 + 0.5*np.tanh(6*(t - 0.5))/np.tanh(3), 0, 1)
pal = np.array([[0.05, 0.28, 0.90], [0.05, 0.95, 0.70], [1.00, 0.70, 0.15]])
col = np.where(t[:, None] < 0.5, pal[0] + (pal[1]-pal[0])*(2*t[:, None]), pal[1] + (pal[2]-pal[1])*(2*t[:, None] - 1))
acc = np.zeros((R, R, 3)); np.add.at(acc, (py[ok], px[ok]), col[ok])
bg = np.array([0.047, 0.047, 0.055]); out = bg + (1 - np.exp(-acc*0.45))*(1 - bg)
art = Image.fromarray((np.clip(out, 0, 1)*255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(0.7))
mask = Image.new('L', (R, R), 0); pad = 72
ImageDraw.Draw(mask).rounded_rectangle([pad, pad, R - pad, R - pad], radius=200, fill=255)
icon = Image.new('RGBA', (R, R), (0, 0, 0, 0)); icon.paste(art, (0, 0), mask)
d = 'src-tauri/icons/'
icon.save(d + 'icon.png')
for sz, name in [(32, '32x32.png'), (128, '128x128.png'), (256, '128x128@2x.png')]:
    icon.resize((sz, sz), Image.LANCZOS).save(d + name)
icon.save(d + 'icon.ico', sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
icon.save(d + 'icon.icns')
print('icons written to', d)
