# Pixel-art conversion for Blender renders: downsample, snap to a hand-built palette, selective outlines.
# Pure numpy so it runs inside Blender's bundled Python.
import numpy as np

# Hue-shifted ramps, dark → light. Shadows lean violet, highlights lean warm.
RAMPS = {
    'steel':  ['#0b0a14', '#1b1830', '#2e2b47', '#4a4766', '#6f6d8c', '#9a99b3', '#c7c7d9', '#eef0f7'],
    'violet': ['#120c2b', '#1e1545', '#2e2063', '#42308a', '#5b45b0', '#7d68d4', '#a596ec'],
    'pink':   ['#3a0a2a', '#6b1048', '#a81c66', '#e0337f', '#ff6aa8', '#ffb3d2'],
    'cyan':   ['#0a2340', '#0f3f6b', '#12669a', '#1c95c8', '#49c6ec', '#a3ecff'],
    'gold':   ['#3d1d08', '#7a3d0c', '#b8650f', '#e89a1c', '#ffcc4d', '#fff1a8'],
    'red':    ['#3a0b10', '#74141f', '#b8222f', '#ee4040', '#ff8a7a'],
    'green':  ['#0c2a1c', '#145233', '#1f8a4c', '#3fc46a', '#8ff09e'],
    'wood':   ['#2a160c', '#4d2a16', '#7a4524', '#a8693a', '#d39a62'],
    'white':  ['#ffffff'],
}

def hex_rgb(h):
    h = h.lstrip('#')
    return [int(h[i:i + 2], 16) / 255 for i in (0, 2, 4)]

PALETTE = []          # list of sRGB triples 0..1
DARKER = []           # index of a darker colour in the same ramp (for outlines)
RAMP_OF = []          # ramp number of each palette entry
RAMP_IDX = []         # palette indices of each ramp
for r, (name, ramp) in enumerate(RAMPS.items()):
    base = len(PALETTE)
    RAMP_IDX.append(list(range(base, base + len(ramp))))
    for i, h in enumerate(ramp):
        PALETTE.append(hex_rgb(h))
        DARKER.append(base + max(0, i - 2))
        RAMP_OF.append(r)
PALETTE = np.array(PALETTE, dtype=np.float32)
DARKER = np.array(DARKER)
RAMP_OF = np.array(RAMP_OF)
RAMP_NAMES = list(RAMPS.keys())

def srgb_to_linear(c):
    return np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)

def to_oklab(srgb):
    l = srgb_to_linear(np.clip(srgb, 0, 1))
    m1 = np.array([[0.4122214708, 0.5363325363, 0.0514459929],
                   [0.2119034982, 0.6806995451, 0.1073969566],
                   [0.0883024619, 0.2817188376, 0.6299787005]], dtype=np.float32)
    m2 = np.array([[0.2104542553, 0.7936177850, -0.0040720468],
                   [1.9779984951, -2.4285922050, 0.4505937099],
                   [0.0259040371, 0.7827717662, -0.8086757660]], dtype=np.float32)
    lms = np.cbrt(l @ m1.T)
    return lms @ m2.T

PAL_LAB = to_oklab(PALETTE)

BAYER4 = (np.array([[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]], dtype=np.float32) + 0.5) / 16 - 0.5

def quantize(rgb, dither=0.0):
    """rgb: (h, w, 3) sRGB → palette indices (h, w). Optional light ordered dither."""
    h, w = rgb.shape[:2]
    if dither > 0:
        tile = np.tile(BAYER4, (h // 4 + 1, w // 4 + 1))[:h, :w]
        rgb = rgb + tile[..., None] * dither
    lab = to_oklab(rgb.reshape(-1, 3).astype(np.float32))
    out = np.empty(lab.shape[0], np.int64)
    wgt = np.array([1.4, 1.0, 1.0], dtype=np.float32)  # weight lightness so ramps stay readable
    for i in range(0, lab.shape[0], 65536):
        d = ((lab[i:i + 65536, None, :] - PAL_LAB[None, :, :]) * wgt) ** 2
        out[i:i + 65536] = d.sum(-1).argmin(-1)
    return out.reshape(h, w)

def nearest_ramp(srgb):
    """ramp number whose colours best match an sRGB colour (used to assign materials to ramps)"""
    lab = to_oklab(np.array([srgb], dtype=np.float32))
    d = ((PAL_LAB - lab) ** 2).sum(-1)
    return int(RAMP_OF[d.argmin()])

def quantize_ramped(rgb, ramps):
    """Snap each pixel to the closest shade within its own material ramp, so shading never changes hue family."""
    h, w = rgb.shape[:2]
    lab = to_oklab(rgb.reshape(-1, 3).astype(np.float32))
    rr = ramps.reshape(-1)
    out = np.zeros(lab.shape[0], np.int64)
    wgt = np.array([2.0, 0.6, 0.6], dtype=np.float32)  # match lightness first: it carries the shading
    for r in np.unique(rr):
        sel = np.nonzero(rr == r)[0]
        cand = np.array(RAMP_IDX[int(r) if 0 <= r < len(RAMP_IDX) else 0])
        for i in range(0, len(sel), 65536):
            s = sel[i:i + 65536]
            d = (((lab[s, None, :] - PAL_LAB[None, cand, :]) * wgt) ** 2).sum(-1)
            out[s] = cand[d.argmin(-1)]
    return out.reshape(h, w)

def decode_ids(px):
    """ID-pass PNG pixels (sRGB-encoded emission of 5-bit channels) → (object id, ramp) arrays; background → -1"""
    lin = srgb_to_linear(np.clip(px[..., :3], 0, 1))
    q = np.round(lin * 31).astype(np.int64)
    idx = q[..., 0] + q[..., 1] * 32 + q[..., 2] * 1024
    bg = px[..., 3] < 0.5
    oid = np.where(bg, -1, idx // 16)
    ramp = np.where(bg, -1, idx % 16)
    return oid, ramp

def downsample(rgba, k):
    """Box-filter a (H, W, 4) straight-alpha image by k, premultiplied so edges stay clean."""
    H, W = rgba.shape[:2]
    h, w = H // k, W // k
    rgba = rgba[: h * k, : w * k]
    a = rgba[..., 3]
    pm = rgba[..., :3] * a[..., None]
    pm = pm.reshape(h, k, w, k, 3).mean((1, 3))
    am = a.reshape(h, k, w, k).mean((1, 3))
    col = pm / np.maximum(am, 1e-6)[..., None]
    return col, am

def mode_downsample(ids, k):
    """Most common id per k×k block (ids: (H, W) int)."""
    H, W = ids.shape
    h, w = H // k, W // k
    blocks = ids[: h * k, : w * k].reshape(h, k, w, k).transpose(0, 2, 1, 3).reshape(h, w, k * k)
    # majority by sorting
    # the median of the sorted block is the majority id whenever one covers half the block
    s = np.sort(blocks, axis=-1)
    return s[..., (k * k) // 2]

def pixelize(rgba, k, ids=None, ramps=None, alpha='hard', dither=0.0, outer_outline=False, inner_outline=True):
    """Returns (h, w, 4) uint8 sRGB pixel art. ids/ramps are full-resolution per-pixel object ids and material ramps."""
    col, am = downsample(rgba, k)
    if ramps is not None:
        rm = mode_downsample(ramps, k)
        idx = quantize_ramped(col, rm)
    else:
        idx = quantize(col, dither)
    h, w = idx.shape
    if alpha == 'hard':
        A = (am > 0.5).astype(np.float32)
    elif alpha == 'soft':
        A = np.round(np.clip(am, 0, 1) * 3) / 3
    else:
        A = np.ones_like(am)
    if inner_outline and ids is not None:
        idd = mode_downsample(ids, k)
        edge = np.zeros((h, w), bool)
        # one-sided: compare with the pixel above and to the left so lines stay 1px wide
        edge[1:, :] |= idd[1:, :] != idd[:-1, :]
        edge[:, 1:] |= idd[:, 1:] != idd[:, :-1]
        edge &= A > 0
        idx = np.where(edge, DARKER[idx], idx)
    out = np.zeros((h, w, 4), np.float32)
    out[..., :3] = PALETTE[idx]
    out[..., 3] = A
    if outer_outline:
        solid = A > 0.5
        ring = np.zeros_like(solid)
        ring[1:, :] |= solid[:-1, :]
        ring[:-1, :] |= solid[1:, :]
        ring[:, 1:] |= solid[:, :-1]
        ring[:, :-1] |= solid[:, 1:]
        ring &= ~solid
        out[ring, :3] = PALETTE[0]
        out[ring, 3] = 1.0
    return (np.clip(out, 0, 1) * 255 + 0.5).astype(np.uint8)
