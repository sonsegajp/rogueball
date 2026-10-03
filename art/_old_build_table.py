# Builds the pinball table in Blender from build/layout.json and exports assets/table.glb (+ table.blend).
# Run:  blender --background --python tools/build_table.py
# Every surface the ball can touch is modeled at its collision size (see src/physics.js) so what you see is what it hits.
import bpy, bmesh, json, math, os

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
L = json.load(open(os.path.join(ROOT, 'build', 'layout.json')))
BALL_R = L['ballR']

bpy.ops.wm.read_factory_settings(use_empty=True)
coll = bpy.context.scene.collection

# ================================================================ materials
MATS = {}
def mat(name, color, metal=0.0, rough=0.5, emit=None, strength=0.0, alpha=1.0):
    if name in MATS:
        return MATS[name]
    m = bpy.data.materials.new(name)
    try:
        m.use_nodes = True
    except Exception:
        pass
    nt = m.node_tree
    p = next((n for n in nt.nodes if n.type == 'BSDF_PRINCIPLED'), None)
    if p is None:
        p = nt.nodes.new('ShaderNodeBsdfPrincipled')
        out = next((n for n in nt.nodes if n.type == 'OUTPUT_MATERIAL'), None) or nt.nodes.new('ShaderNodeOutputMaterial')
        nt.links.new(p.outputs[0], out.inputs[0])
    p.inputs['Base Color'].default_value = (*color, 1)
    p.inputs['Metallic'].default_value = metal
    p.inputs['Roughness'].default_value = rough
    if emit:
        p.inputs['Emission Color'].default_value = (*emit, 1)
        p.inputs['Emission Strength'].default_value = strength
    if alpha < 1:
        p.inputs['Alpha'].default_value = alpha
        for attr, val in (('surface_render_method', 'BLENDED'), ('blend_method', 'BLEND')):
            try:
                setattr(m, attr, val)
            except Exception:
                pass
    m.diffuse_color = (*color, alpha)
    MATS[name] = m
    return m

WARM = (1.0, 0.72, 0.42)
PINK = (1.0, 0.1, 0.45)
CYAN = (0.1, 0.7, 1.0)
GOLD = (1.0, 0.62, 0.1)

M = {
    'playfield':  mat('playfield', (0.05, 0.04, 0.09), rough=0.2),
    'wood':       mat('wood_edge', (0.36, 0.22, 0.12), rough=0.6),
    'chrome':     mat('chrome', (0.95, 0.95, 0.97), metal=1.0, rough=0.08),
    'steel':      mat('steel', (0.62, 0.63, 0.66), metal=1.0, rough=0.32),
    'black':      mat('black_metal', (0.025, 0.025, 0.03), metal=0.4, rough=0.45),
    'rubber':     mat('rubber', (0.02, 0.02, 0.022), rough=0.75),
    'rubber_red': mat('rubber_red', (0.65, 0.04, 0.08), rough=0.6),
    'flip':       mat('flipper_body', (0.93, 0.93, 0.95), rough=0.35),
    'wire':       mat('wire', (0.75, 0.76, 0.8), metal=1.0, rough=0.2),
    'ramp':       mat('ramp_plastic', (0.55, 0.85, 1.0), rough=0.05, alpha=0.35),
    'plastic':    mat('plastic_clear', (0.9, 0.9, 1.0), rough=0.05, alpha=0.55),
    'bump_base':  mat('bumper_base', (0.03, 0.03, 0.035), rough=0.4),
    'bump_skirt': mat('bumper_skirt', (0.75, 0.05, 0.12), rough=0.35),
    'bump_body':  mat('bumper_body', (0.95, 0.95, 1.0), rough=0.2, alpha=0.75),
    'target_w':   mat('target_stripe', (0.95, 0.95, 0.95), rough=0.4),
    'gi':         mat('gi_bulb', WARM, rough=0.2, emit=WARM, strength=3.0),
    'cabinet':    mat('cabinet', (0.03, 0.02, 0.05), rough=0.45, metal=0.1),
    'apron':      mat('apron', (0.07, 0.05, 0.12), rough=0.4, metal=0.3),
    'dmd':        mat('dmd', (0.0, 0.0, 0.0), rough=0.9),
    'backglass':  mat('backglass', (0.0, 0.0, 0.0), rough=0.2),
    'grille':     mat('speaker_grille', (0.02, 0.02, 0.02), rough=0.8, metal=0.2),
    'button':     mat('button_red', (0.8, 0.05, 0.08), rough=0.25, emit=(1, 0.1, 0.1), strength=0.6),
    'coin':       mat('coin_light', (0.8, 0.1, 0.1), emit=(1, 0.15, 0.1), strength=1.2),
    'accent':     mat('neon_accent', PINK, emit=PINK, strength=1.2),
    'card':       mat('apron_card', (1, 1, 1), rough=0.6),
}

# ================================================================ mesh helpers
def make(name, verts, faces, m, smooth=None, recalc=True, parent=None, loc=(0, 0, 0)):
    me = bpy.data.meshes.new(name)
    me.from_pydata([tuple(v) for v in verts], [], [tuple(f) for f in faces])
    me.update()
    if recalc:
        bm = bmesh.new()
        bm.from_mesh(me)
        bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
        bm.to_mesh(me)
        bm.free()
    if smooth is not None:
        flags = [bool(smooth(i)) for i in range(len(me.polygons))] if callable(smooth) else [bool(smooth)] * len(me.polygons)
        me.polygons.foreach_set('use_smooth', flags)
    ob = bpy.data.objects.new(name, me)
    coll.objects.link(ob)
    if m:
        me.materials.append(m)
    ob.location = loc
    if parent:
        ob.parent = parent
    return ob

def area(poly):
    a = 0
    for i in range(len(poly)):
        x0, y0 = poly[i][:2]; x1, y1 = poly[(i + 1) % len(poly)][:2]
        a += x0 * y1 - x1 * y0
    return a / 2

def prism(name, poly, z0, z1, m, **kw):
    poly = [tuple(p[:2]) for p in poly]
    if area(poly) < 0:
        poly = poly[::-1]
    n = len(poly)
    verts = [(x, y, z0) for x, y in poly] + [(x, y, z1) for x, y in poly]
    faces = [tuple(range(n - 1, -1, -1)), tuple(range(n, 2 * n))]
    for i in range(n):
        j = (i + 1) % n
        faces.append((i, j, n + j, n + i))
    return make(name, verts, faces, m, **kw)

def sweep(name, pts, r, zb, zt, m, closed=False, round_top=False, **kw):
    """Rail swept along a 2D polyline; pts may carry per-point [x, y, zb, zt]. round_top adds a rounded crown."""
    n = len(pts)
    def d(i, j):
        dx = pts[j][0] - pts[i][0]; dy = pts[j][1] - pts[i][1]
        l = math.hypot(dx, dy) or 1
        return dx / l, dy / l
    prof = [(1, 0), (-1, 0)]
    crown = [(-1, 0), (-0.7, 0.7), (0, 1), (0.7, 0.7), (1, 0)] if round_top else [(-1, 0), (1, 0)]
    verts = []
    per = 2 + len(crown)
    for i in range(n):
        if closed:
            a = d((i - 1) % n, i); b = d(i, (i + 1) % n)
        else:
            a = d(max(i - 1, 0), max(i, 1)) if i > 0 else d(0, 1)
            b = d(i, min(i + 1, n - 1)) if i < n - 1 else a
        n0 = (-a[1], a[0]); n1 = (-b[1], b[0])
        mx, my = n0[0] + n1[0], n0[1] + n1[1]
        ml = math.hypot(mx, my) or 1
        mx, my = mx / ml, my / ml
        k = mx * n1[0] + my * n1[1]
        s = min(r / max(k, 0.2), r * 3)
        x, y = pts[i][0], pts[i][1]
        b0 = pts[i][2] if len(pts[i]) > 2 else zb
        t0 = pts[i][3] if len(pts[i]) > 3 else zt
        # bottom right, bottom left, then the crown from left to right
        verts.append((x - mx * s, y - my * s, b0))
        verts.append((x + mx * s, y + my * s, b0))
        for cx, cz in crown:
            verts.append((x - mx * s * cx, y - my * s * cx, t0 - (r if round_top else 0) + cz * (r if round_top else 0)))
    # cross-section loop: bottom-right(0) → crown from the right side to the left → bottom-left(1)
    ring = [0] + [2 + j for j in reversed(range(len(crown)))] + [1]
    faces = []
    segs = n if closed else n - 1
    for i in range(segs):
        a = i * per; b = ((i + 1) % n) * per
        for j in range(len(ring)):
            p, q = ring[j], ring[(j + 1) % len(ring)]
            faces.append((a + p, b + p, b + q, a + q))
    if not closed:
        faces.append(tuple(ring[::-1]))
        faces.append(tuple((n - 1) * per + j for j in ring))
    return make(name, verts, faces, m, smooth=(lambda f: round_top) if round_top else None, **kw)

def cylinder(name, x, y, r, z0, z1, m, seg=32, r_top=None, **kw):
    rt = r if r_top is None else r_top
    verts, faces = [], []
    for i in range(seg):
        a = 2 * math.pi * i / seg
        c, s = math.cos(a), math.sin(a)
        verts += [(x + c * r, y + s * r, z0), (x + c * rt, y + s * rt, z1)]
    for i in range(seg):
        j = (i + 1) % seg
        faces.append((i * 2, j * 2, j * 2 + 1, i * 2 + 1))
    side = len(faces)
    base = len(verts)
    for i in range(seg):
        a = 2 * math.pi * i / seg
        verts.append((x + math.cos(a) * r, y + math.sin(a) * r, z0))
    for i in range(seg):
        a = 2 * math.pi * i / seg
        verts.append((x + math.cos(a) * rt, y + math.sin(a) * rt, z1))
    faces.append(tuple(range(base + seg - 1, base - 1, -1)))
    faces.append(tuple(range(base + seg, base + 2 * seg)))
    return make(name, verts, faces, m, smooth=(lambda i: i < side) if seg > 8 else None, **kw)

def dome(name, x, y, r, z0, h, m, seg=32, rings=8, **kw):
    verts, faces = [], []
    for j in range(rings + 1):
        t = j / rings * math.pi / 2
        rr = max(r * math.cos(t), 1e-5); zz = z0 + h * math.sin(t)
        for i in range(seg):
            a = 2 * math.pi * i / seg
            verts.append((x + math.cos(a) * rr, y + math.sin(a) * rr, zz))
    for j in range(rings):
        for i in range(seg):
            k = (i + 1) % seg
            faces.append((j * seg + i, j * seg + k, (j + 1) * seg + k, (j + 1) * seg + i))
    faces.append(tuple(range(seg - 1, -1, -1)))
    return make(name, verts, faces, m, smooth=lambda i: i < rings * seg, **kw)

def torus(name, x, y, z, R, r, m, seg=36, rseg=10, **kw):
    verts, faces = [], []
    for i in range(seg):
        a = 2 * math.pi * i / seg
        for j in range(rseg):
            b = 2 * math.pi * j / rseg
            rr = R + r * math.cos(b)
            verts.append((x + rr * math.cos(a), y + rr * math.sin(a), z + r * math.sin(b)))
    for i in range(seg):
        for j in range(rseg):
            i2, j2 = (i + 1) % seg, (j + 1) % rseg
            faces.append((i * rseg + j, i2 * rseg + j, i2 * rseg + j2, i * rseg + j2))
    return make(name, verts, faces, m, smooth=True, **kw)

def v_sub(a, b): return (a[0] - b[0], a[1] - b[1], a[2] - b[2])
def v_add(a, b): return (a[0] + b[0], a[1] + b[1], a[2] + b[2])
def v_mul(a, k): return (a[0] * k, a[1] * k, a[2] * k)
def v_dot(a, b): return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
def v_cross(a, b): return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])
def v_norm(a):
    l = math.sqrt(v_dot(a, a)) or 1
    return (a[0] / l, a[1] / l, a[2] / l)

def tube(name, pts, r, m, seg=8, **kw):
    """Round wire along a 3D polyline (parallel-transported frame)."""
    n = len(pts)
    T = [v_norm(v_sub(pts[min(i + 1, n - 1)], pts[max(i - 1, 0)])) for i in range(n)]
    up = (0, 0, 1) if abs(T[0][2]) < 0.9 else (1, 0, 0)
    N = [v_norm(v_cross(v_cross(T[0], up), T[0]))]
    for i in range(1, n):
        prev = N[-1]
        N.append(v_norm(v_sub(prev, v_mul(T[i], v_dot(prev, T[i])))))
    verts, faces = [], []
    for i in range(n):
        B = v_cross(T[i], N[i])
        for k in range(seg):
            a = 2 * math.pi * k / seg
            verts.append(v_add(pts[i], v_add(v_mul(N[i], math.cos(a) * r), v_mul(B, math.sin(a) * r))))
    for i in range(n - 1):
        for k in range(seg):
            k2 = (k + 1) % seg
            faces.append((i * seg + k, i * seg + k2, (i + 1) * seg + k2, (i + 1) * seg + k))
    side = len(faces)
    faces.append(tuple(range(seg - 1, -1, -1)))
    faces.append(tuple((n - 1) * seg + k for k in range(seg)))
    return make(name, verts, faces, m, smooth=lambda i: i < side, **kw)

def box(name, cx, cy, z0, w, d, h, m, rot=0.0, **kw):
    c, s = math.cos(rot), math.sin(rot)
    pts = [(-w / 2, -d / 2), (w / 2, -d / 2), (w / 2, d / 2), (-w / 2, d / 2)]
    poly = [(cx + px * c - py * s, cy + px * s + py * c) for px, py in pts]
    return prism(name, poly, z0, z0 + h, m, **kw)

def hull(points):
    pts = sorted(set((round(p[0], 6), round(p[1], 6)) for p in points))
    def cross(o, a, b):
        return (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    lo, hi = [], []
    for p in pts:
        while len(lo) >= 2 and cross(lo[-2], lo[-1], p) <= 0:
            lo.pop()
        lo.append(p)
    for p in reversed(pts):
        while len(hi) >= 2 and cross(hi[-2], hi[-1], p) <= 0:
            hi.pop()
        hi.append(p)
    return lo[:-1] + hi[:-1]

def circles_hull(centers, r, seg=20):
    pts = []
    for cx, cy in centers:
        for i in range(seg):
            a = 2 * math.pi * i / seg
            pts.append((cx + math.cos(a) * r, cy + math.sin(a) * r))
    return hull(pts)

def capsule_outline(length, r0, r1, seg=28):
    return circles_hull([(0, 0)], r0, seg) if length == 0 else hull(
        [(math.cos(2 * math.pi * i / seg) * r0, math.sin(2 * math.pi * i / seg) * r0) for i in range(seg)] +
        [(length + math.cos(2 * math.pi * i / seg) * r1, math.sin(2 * math.pi * i / seg) * r1) for i in range(seg)])

def nut(name, x, y, z, m=None, r=0.0032, h=0.0025, **kw):
    """hex nut / screw head"""
    return cylinder(name, x, y, r, z, z + h, m or M['chrome'], seg=6, **kw)

def screw(name, x, y, z, **kw):
    return dome(name, x, y, 0.0022, z, 0.0012, M['chrome'], seg=12, rings=3, **kw)

FONT = None
for f in ('C:/Windows/Fonts/bahnschrift.ttf', 'C:/Windows/Fonts/impact.ttf'):
    if os.path.exists(f):
        try:
            FONT = bpy.data.fonts.load(f)
            break
        except Exception:
            pass

def quad_uv(name, corners, m, **kw):
    ob = make(name, corners, [(0, 1, 2, 3)], m, recalc=False, **kw)
    uv = ob.data.uv_layers.new(name='UVMap')
    for li, (u, v) in enumerate([(0, 0), (1, 0), (1, 1), (0, 1)]):
        uv.data[li].uv = (u, v)
    return ob

# ================================================================ playfield
outline = L['outline']
prism('playfield', outline, -0.003, 0.0, M['playfield'])
prism('playfield_wood', outline, -0.019, -0.003, M['wood'])

# ================================================================ walls & guides
for i, w in enumerate(L['walls']):
    vis = w.get('vis', 'rail')
    if vis == 'outer':
        # the inner face of the cabinet wall is where the ball bounces; dark painted wood with a steel cap
        sweep('wall_outer', w['pts'], w['r'], 0.0, w['zt'], M['cabinet'])
        sweep('wall_outer_cap', [p[:2] for p in w['pts']], w['r'] + 0.0015, w['zt'], w['zt'] + 0.004, M['steel'], round_top=True)
    elif vis == 'rampwall':
        continue  # built with the ramp
    else:
        sweep('guide_%d' % i, w['pts'], w['r'], w['zb'], w['zt'], M['steel'], round_top=True)
        # screws along longer guides
        p0, p1 = w['pts'][0], w['pts'][-1]
        if math.hypot(p1[0] - p0[0], p1[1] - p0[1]) > 0.06:
            for k, t in enumerate((0.25, 0.75)):
                screw('guide_%d_screw%d' % (i, k), p0[0] + (p1[0] - p0[0]) * t, p0[1] + (p1[1] - p0[1]) * t, w['zt'])

for i, p in enumerate(L['posts']):
    x, y, r, zt = p['x'], p['y'], p['r'], p['zt']
    if p['mat'] == 'rubber':
        # star post: steel post, black rubber ring whose outer edge is the collision radius, nut on top
        cylinder('post_%d' % i, x, y, 0.0028, 0, zt, M['steel'], seg=16)
        torus('post_ring_%d' % i, x, y, BALL_R, r - 0.0028, 0.0028, M['rubber'])
        nut('post_nut_%d' % i, x, y, zt)
    else:
        cylinder('post_%d' % i, x, y, r, 0, zt, M['steel'], seg=24)
        nut('post_nut_%d' % i, x, y, zt, r=r * 0.7)

# ================================================================ slingshots
for s in L['slings']:
    pts, r, sid = s['pts'], s['r'], s['id']
    # rubber band: the hull of the three posts at the collision radius
    band = circles_hull([tuple(p) for p in pts], r - 0.0018, seg=24)
    sweep('sling_%s_rubber' % sid, band, 0.0018, BALL_R - 0.0055, BALL_R + 0.0055, M['rubber'], closed=True)
    for j, p in enumerate(pts):
        cylinder('sling_%s_post%d' % (sid, j), p[0], p[1], 0.0028, 0, 0.046, M['steel'], seg=12)
        nut('sling_%s_nut%d' % (sid, j), p[0], p[1], 0.049)
    # kicker arm behind the kicking rubber
    a, c = pts[s['kick'][0]], pts[s['kick'][1]]
    cx = sum(p[0] for p in pts) / 3; cy = sum(p[1] for p in pts) / 3
    mxk, myk = (a[0] + c[0]) / 2, (a[1] + c[1]) / 2
    ang = math.atan2(c[1] - a[1], c[0] - a[0])
    box('sling_%s_arm' % sid, mxk + (cx - mxk) * 0.35, myk + (cy - myk) * 0.35, 0.004, 0.03, 0.004, 0.012, M['black'], rot=ang)
    # GI bulb under the plastic
    dome('sling_%s_bulb' % sid, cx, cy, 0.004, 0.0, 0.009, M['gi'], seg=12, rings=4)
    # clear plastic with printed art (art and UVs come from the game)
    grown = circles_hull([tuple(p) for p in pts], r + 0.006, seg=16)
    prism('plastic_sling_%s' % sid, grown, 0.046, 0.0485, M['plastic'])
    # lit edge strip on the kicking side, flashes when the sling fires
    glow = mat('sling_' + sid, PINK, rough=0.3, emit=PINK, strength=0.5)
    nx, ny = -(c[1] - a[1]), c[0] - a[0]
    nl = math.hypot(nx, ny); nx /= nl; ny /= nl
    if (mxk + nx - cx) ** 2 + (myk + ny - cy) ** 2 < (mxk - cx) ** 2 + (myk - cy) ** 2:
        nx, ny = -nx, -ny
    off = r + 0.004
    sweep('sling_%s_glow' % sid, [(a[0] + nx * off, a[1] + ny * off), (c[0] + nx * off, c[1] + ny * off)], 0.0012, 0.0485, 0.0495, glow)

# ================================================================ flippers
for f in L['flippers']:
    fid = f['id']
    body = prism('flipper_%s' % fid, capsule_outline(f['len'], f['r0'] - 0.003, f['r1'] - 0.003), 0.002, f['h'], M['flip'])
    body.location = (f['x'], f['y'], 0)
    body.rotation_euler = (0, 0, math.radians(f['rest']))
    # the rubber's outer edge is the collision outline
    prism('flipper_%s_rubber' % fid, capsule_outline(f['len'], f['r0'], f['r1']), 0.007, 0.019, M['rubber_red'], parent=body)
    cylinder('flipper_%s_cap' % fid, 0, 0, 0.0065, f['h'], f['h'] + 0.002, M['chrome'], seg=24, parent=body)
    nut('flipper_%s_bolt' % fid, 0, 0, f['h'] + 0.002, r=0.003, parent=body)

# ================================================================ pop bumpers
BUMP_COL = {'b1': CYAN, 'b2': PINK, 'b3': GOLD}
for b in L['bumpers']:
    x, y, r, bid = b['x'], b['y'], b['r'], b['id']
    col = BUMP_COL.get(bid, CYAN)
    cylinder('bumper_%s_base' % bid, x, y, r + 0.004, 0, 0.003, M['bump_base'])
    cylinder('bumper_%s_skirt' % bid, x, y, r, 0.003, 0.009, M['bump_skirt'], r_top=r - 0.006)
    cylinder('bumper_%s_body' % bid, x, y, r * 0.74, 0.009, 0.033, M['bump_body'])
    ring = cylinder('bumper_%s_ring' % bid, 0, 0, r + 0.0005, 0.015, 0.0195, M['chrome'], r_top=r + 0.0005)
    ring.location = (x, y, 0)
    for k in range(3):
        a = 2 * math.pi * k / 3 + 0.5
        cylinder('bumper_%s_rod%d' % (bid, k), x + math.cos(a) * r * 0.82, y + math.sin(a) * r * 0.82, 0.0012, 0.009, 0.033, M['steel'], seg=8)
    capm = mat('bumper_' + bid, tuple(0.35 + 0.5 * c for c in col), rough=0.15, emit=col, strength=0.6)
    cap = cylinder('bumper_%s_cap' % bid, 0, 0, r + 0.002, 0.033, 0.038, capm)
    cap.location = (x, y, 0)
    dome('bumper_%s_dome' % bid, 0, 0, r * 0.72, 0.038, 0.005, capm, parent=cap)
    cylinder('bumper_%s_logo' % bid, 0, 0, r * 0.38, 0.043, 0.0434, M['target_w'], seg=5, parent=cap)

# ================================================================ drop targets & standups
for d in L['drops']:
    (ax, ay), (bx, by) = d['a'], d['b']
    cx, cy = (ax + bx) / 2, (ay + by) / 2
    w = math.hypot(bx - ax, by - ay)
    rot = math.atan2(by - ay, bx - ax)
    t = box('drop_%s' % d['id'], 0, 0, 0, w - 0.002, 2 * d['r'] - 0.001, d['zt'], mat('drop_' + d['id'], GOLD, rough=0.35, emit=GOLD, strength=0.25), rot=rot)
    t.location = (cx, cy, 0)
    box('drop_%s_stripe' % d['id'], 0, -d['r'], 0.012, w * 0.7, 0.0008, 0.006, M['target_w'], rot=rot, parent=t)
    box('drop_%s_slot' % d['id'], cx, cy, -0.0004, w + 0.002, 2 * d['r'] + 0.003, 0.0006, M['black'], rot=rot)

for s in L['standups']:
    (ax, ay), (bx, by) = s['a'], s['b']
    cx, cy = (ax + bx) / 2, (ay + by) / 2
    w = math.hypot(bx - ax, by - ay)
    rot = math.atan2(by - ay, bx - ax)
    nx, ny = s['n']
    t = box('stand_%s' % s['id'], 0, 0, 0.006, w, 2 * s['r'] - 0.002, s['zt'] - 0.006, mat('stand_' + s['id'], PINK, rough=0.35, emit=PINK, strength=0.25), rot=rot)
    t.location = (cx, cy, 0)
    box('stand_%s_stripe' % s['id'], nx * s['r'] * 0.95, ny * s['r'] * 0.95, 0.016, w * 0.6, 0.0008, 0.005, M['target_w'], rot=rot, parent=t)
    box('stand_%s_base' % s['id'], cx - nx * 0.009, cy - ny * 0.009, 0, 0.014, 0.012, 0.03, M['black'], rot=rot)

# ================================================================ spinner, rollovers, gate
for sp in L['spinners']:
    (ax, ay), (bx, by) = sp['a'], sp['b']
    z = sp['z']
    cx, cy = (ax + bx) / 2, (ay + by) / 2
    w = abs(bx - ax) - 0.005
    plate = box('spinner_%s' % sp['id'], 0, 0, -0.022, w, 0.0012, 0.02, M['chrome'])
    plate.location = (cx, cy, z)
    box('spinner_%s_decal' % sp['id'], 0, -0.0007, -0.019, w * 0.8, 0.0004, 0.014, mat('spinner_decal', (0.9, 0.15, 0.35), rough=0.4), parent=plate)
    for k, px in enumerate((ax, bx)):
        tube('spinner_%s_bracket%d' % (sp['id'], k), [(px, ay, -0.001), (px, ay, z + 0.004), (px + (0.002 if k == 0 else -0.002), ay, z + 0.004)], 0.0012, M['wire'])
    tube('spinner_%s_axle' % sp['id'], [(ax, ay, z), (bx, by, z)], 0.0009, M['steel'])

for ro in L['rollovers']:
    x, y = ro['x'], ro['y']
    box('roll_%s_slot' % ro['id'], x, y, -0.0004, 0.004, 0.026, 0.0006, M['black'])
    tube('roll_%s' % ro['id'], [(x, y - 0.012, -0.001), (x, y - 0.006, 0.004), (x, y, 0.0055), (x, y + 0.006, 0.004), (x, y + 0.012, -0.001)], 0.0008, M['wire'], seg=6)

for g in L['gates']:
    (ax, ay), (bx, by) = g['a'], g['b']
    mxg = (ax + bx) / 2
    flap = tube('gate_%s' % g['id'], [(ax - mxg + 0.002, 0, 0), (ax - mxg + 0.002, 0, -0.026), (bx - mxg - 0.002, 0, -0.026), (bx - mxg - 0.002, 0, 0)], 0.0008, M['wire'], seg=6)
    flap.location = (mxg, ay, 0.04)
    for k, px in enumerate((ax, bx)):
        tube('gate_%s_bracket%d' % (g['id'], k), [(px, ay, 0), (px, ay, 0.041)], 0.0012, M['wire'])
    tube('gate_%s_axle' % g['id'], [(ax, ay, 0.04), (bx, by, 0.04)], 0.0008, M['steel'])

# ================================================================ ramp: clear plastic up-ramp, wire-form return
for rp in L['ramps']:
    path, split, solid = rp['path'], rp['split'], rp['solidFrom']
    hw = rp['halfW']
    left = [w for w in L['walls'] if w.get('vis') == 'rampwall'][0]['pts']
    right = [w for w in L['walls'] if w.get('vis') == 'rampwall'][1]['pts']
    # plastic floor up to the U-turn
    pv = rp['verts'][: (split + 1) * 2]
    pf = [(i * 2, i * 2 + 1, i * 2 + 3, i * 2 + 2) for i in range(split)]
    make('ramp_floor', pv, pf, M['ramp'], recalc=False)
    sweep('ramp_wall_l', left[: split + 1], 0.0015, 0, 0, M['ramp'])
    sweep('ramp_wall_r', right[: split + 1], 0.0015, 0, 0, M['ramp'])
    tube('ramp_lip_l', [(p[0], p[1], p[3]) for p in left[: split + 1]], 0.0018, M['steel'])
    tube('ramp_lip_r', [(p[0], p[1], p[3]) for p in right[: split + 1]], 0.0018, M['steel'])
    p0 = path[0]
    box('ramp_flap', p0[0], p0[1] - 0.005, -0.0003, hw * 2 + 0.012, 0.012, 0.001, M['steel'])
    # wire-form: two bottom wires the ball rolls on, two side wires, a top guard on the outside of the turn
    def wire_at(i, lat, dz):
        p = path[i]
        a = path[max(0, i - 1)]; b = path[min(len(path) - 1, i + 1)]
        tx, ty = b[0] - a[0], b[1] - a[1]
        tl = math.hypot(tx, ty) or 1
        nx, ny = -ty / tl, tx / tl
        return (p[0] + nx * lat, p[1] + ny * lat, p[2] + dz)
    idx = list(range(split - 1, len(path)))
    lift = BALL_R - math.sqrt(BALL_R ** 2 - 0.0095 ** 2) - 0.0012
    for name, lat, dz in (('wl', 0.0095, lift), ('wr', -0.0095, lift), ('sl', hw + 0.003, 0.012), ('sr', -(hw + 0.003), 0.012), ('tl', hw + 0.003, 0.026), ('tr', -(hw + 0.003), 0.026)):
        tube('ramp_wire_' + name, [wire_at(i, lat, dz) for i in idx], 0.0012, M['wire'], seg=8)
    # hoops tying the wires together, and support posts
    for k, i in enumerate(idx[1::3]):
        hoop = [wire_at(i, (hw + 0.003) * math.cos(a), 0.012 - 0.012 * math.sin(a)) for a in [math.pi * j / 10 for j in range(11)]]
        tube('ramp_hoop_%d' % k, hoop, 0.0009, M['wire'], seg=6)
        p = path[i]
        if p[2] > 0.04 and i < solid:
            tube('ramp_support_%d' % k, [(p[0], p[1], 0), (p[0], p[1], p[2] - 0.0005)], 0.0016, M['steel'])
            cylinder('ramp_support_foot_%d' % k, p[0], p[1], 0.004, 0, 0.002, M['steel'], seg=12)
    # sheet-metal skirts where the descending end is solid down to the playfield
    if solid < len(path):
        sk_l = [[p[0], p[1], 0, max(p[3] - 0.03 + 0.002, 0.004)] for p in left[solid:]]
        sk_r = [[p[0], p[1], 0, max(p[3] - 0.03 + 0.002, 0.004)] for p in right[solid:]]
        sweep('ramp_skirt_l', sk_l, 0.0015, 0, 0, M['black'])
        sweep('ramp_skirt_r', sk_r, 0.0015, 0, 0, M['black'])
        pf = path[solid]
        box('ramp_skirt_cap', pf[0], pf[1], 0, hw * 2 + 0.006, 0.003, pf[2] - 0.002, M['black'], rot=math.atan2(path[solid + 1][1] - pf[1], path[solid + 1][0] - pf[0]) + math.pi / 2)

# ================================================================ inserts & GI
def insert_shape(ins):
    x, y, r = ins['x'], ins['y'], ins['r']
    rot = math.radians(ins.get('rot', 0))
    if ins['shape'] == 'arrow':
        local = [(r, 0), (-r * 0.6, r * 0.7), (-r * 0.25, 0), (-r * 0.6, -r * 0.7)]
    elif ins['shape'] == 'chevron':
        local = [(r * 0.5, 0), (-r * 0.5, r), (-r * 0.9, r), (0.1 * r, 0), (-r * 0.9, -r), (-r * 0.5, -r)]
    else:
        local = [(math.cos(a) * r, math.sin(a) * r) for a in [2 * math.pi * i / 28 for i in range(28)]]
    c, s = math.cos(rot), math.sin(rot)
    return [(x + px * c - py * s, y + px * s + py * c) for px, py in local]

for ins in L['inserts']:
    col = tuple(ins['color'])
    m = mat('ins_' + ins['id'], tuple(v * 0.35 for v in col), rough=0.08, emit=col, strength=1.0)
    poly = insert_shape(ins)
    if ins['shape'] == 'chevron':
        make('ins_%s' % ins['id'], [(p[0], p[1], 0.0003) for p in poly], [(0, 1, 2, 3), (0, 3, 4, 5)], m, recalc=False)
    else:
        prism('ins_%s' % ins['id'], poly, -0.0015, 0.0003, m)

for k, (x, y) in enumerate(L.get('gi', [])):
    cylinder('gi_socket_%d' % k, x, y, 0.004, 0, 0.003, M['black'], seg=10)
    dome('gi_bulb_%d' % k, x, y, 0.0035, 0.003, 0.008, M['gi'], seg=12, rings=4)

# ================================================================ apron, shooter lane, plunger
ap = L['apron']
x0a, x1a, y0a, y1a = ap['x0'], ap['x1'], ap['y0'], ap['y1']
prism('apron', [(x0a, y0a), (x1a, y0a), (x1a, y1a - 0.014), (x1a - 0.035, y1a), (x0a + 0.035, y1a), (x0a, y1a - 0.014)], 0, 0.028, M['apron'])
tube('apron_lip', [(x0a, y1a - 0.014, 0.028), (x0a + 0.035, y1a, 0.028), (x1a - 0.035, y1a, 0.028), (x1a, y1a - 0.014, 0.028)], 0.0016, M['steel'])
quad_uv('apron_card_l', [(-0.205, 0.004, 0.0283), (-0.065, 0.004, 0.0283), (-0.065, 0.042, 0.0283), (-0.205, 0.042, 0.0283)], M['card'])
quad_uv('apron_card_r', [(0.065, 0.004, 0.0283), (0.205, 0.004, 0.0283), (0.205, 0.042, 0.0283), (0.065, 0.042, 0.0283)], M['card'])
for x in (-0.215, -0.055, 0.055, 0.215):
    screw('apron_screw_%d' % int(x * 1000), x, 0.023, 0.028)

pl = L['plunger']
def cyl_y(name, x, y0, y1, z, r, m, seg=20, **kw):
    verts, faces = [], []
    for i in range(seg):
        a = 2 * math.pi * i / seg
        verts += [(x + math.cos(a) * r, y0, z + math.sin(a) * r), (x + math.cos(a) * r, y1, z + math.sin(a) * r)]
    for i in range(seg):
        j = (i + 1) % seg
        faces.append((i * 2, j * 2, j * 2 + 1, i * 2 + 1))
    faces.append(tuple(i * 2 for i in range(seg)))
    faces.append(tuple(i * 2 + 1 for i in range(seg - 1, -1, -1)))
    return make(name, verts, faces, m, smooth=lambda k: k < seg, **kw)
rod = cyl_y('plunger', 0, -0.14, pl['y0'] - 0.004, 0, 0.003, M['chrome'])
rod.location = (pl['x'], 0, BALL_R)
cyl_y('plunger_tip', 0, pl['y0'] - 0.007, pl['y0'], 0, 0.0085, M['rubber'], seg=24, parent=rod)
cyl_y('plunger_knob', 0, -0.185, -0.15, 0, 0.011, M['chrome'], seg=24, parent=rod)
spring = [(0.0055 * math.cos(t * 0.9), -0.12 + t * 0.0016, 0.0055 * math.sin(t * 0.9)) for t in range(0, 40)]
tube('plunger_spring', spring, 0.0008, M['steel'], seg=6, parent=rod)
cylinder('shooter_housing', pl['x'], -0.012, 0.012, -0.01, 0.0, M['steel'])

# ================================================================ cabinet
cx0, cx1 = -0.265, 0.303
cy0, cy1 = -0.115, 1.10
CABZ = -0.27
prism('cabinet_body', [(cx0, cy0), (cx1, cy0), (cx1, cy1), (cx0, cy1)], CABZ, -0.019, M['cabinet'])
# side walls with brushed steel side rails
for side, x in (('l', cx0), ('r', cx1 - 0.022)):
    box('cabinet_side_' + side, x + 0.011, (cy0 + cy1) / 2, CABZ, 0.022, cy1 - cy0, 0.345, M['cabinet'])
    box('side_rail_' + side, x + 0.011, (cy0 + cy1) / 2 + 0.01, 0.075, 0.03, cy1 - cy0 - 0.02, 0.008, M['steel'])
    box('side_rail_trim_' + side, x + 0.011 + (0.0145 if side == 'r' else -0.0145), (cy0 + cy1) / 2 + 0.01, 0.06, 0.002, cy1 - cy0 - 0.02, 0.015, M['chrome'])
# fill between the playfield walls and the cabinet sides
prism('cabinet_fill_l', [(cx0 + 0.02, cy0 + 0.1), (-0.241, cy0 + 0.1), (-0.241, cy1), (cx0 + 0.02, cy1)], -0.019, 0.075, M['cabinet'])
prism('cabinet_fill_r', [(0.279, cy0 + 0.1), (cx1 - 0.02, cy0 + 0.1), (cx1 - 0.02, cy1), (0.279, cy1)], -0.019, 0.075, M['cabinet'])
corner_l = [(-0.241, 0.80)] + [(0.019 + 0.26 * math.cos(math.radians(a)), 0.80 + 0.26 * math.sin(math.radians(a))) for a in range(174, 89, -6)] + [(-0.241, cy1)]
prism('cabinet_corner_l', corner_l, -0.019, 0.075, M['cabinet'])
corner_r = [(0.279, 0.80), (0.279, cy1)] + [(0.019 + 0.26 * math.cos(math.radians(a)), 0.80 + 0.26 * math.sin(math.radians(a))) for a in range(90, 0, -6)]
prism('cabinet_corner_r', corner_r, -0.019, 0.075, M['cabinet'])
box('cabinet_head', 0.019, cy1 - 0.015, CABZ, cx1 - cx0, 0.03, 0.345, M['cabinet'])
# front: lockdown bar, coin door, start and flipper buttons
box('cabinet_front', 0.019, cy0 + 0.05, CABZ, cx1 - cx0, 0.10, 0.26, M['cabinet'])
verts, faces = [], []
for i in range(16):
    a = 2 * math.pi * i / 16
    verts += [(cx0 - 0.005, cy0 + 0.045 + math.cos(a) * 0.022, 0.07 + math.sin(a) * 0.012), (cx1 + 0.005, cy0 + 0.045 + math.cos(a) * 0.022, 0.07 + math.sin(a) * 0.012)]
for i in range(16):
    j = (i + 1) % 16
    faces.append((i * 2, j * 2, j * 2 + 1, i * 2 + 1))
faces.append(tuple(i * 2 for i in range(16)))
faces.append(tuple(i * 2 + 1 for i in range(15, -1, -1)))
make('lockdown_bar', verts, faces, M['chrome'], smooth=lambda k: k < 16)
cylinder('start_button', 0.22, cy0 + 0.045, 0.008, 0.082, 0.087, M['button'], seg=20)
for side, x in (('l', cx0 - 0.004), ('r', cx1 + 0.004)):
    b = cylinder('flipper_button_' + side, 0, 0, 0.011, 0, 0.008, M['button'], seg=20)
    b.location = (x, cy0 + 0.1, -0.06)
    b.rotation_euler = (0, math.radians(90 if side == 'r' else -90), 0)
box('coin_door', 0.019, cy0 - 0.001, -0.26, 0.2, 0.004, 0.17, M['steel'])
for k, x in enumerate((-0.03, 0.068)):
    box('coin_slot_%d' % k, x, cy0 - 0.004, -0.14, 0.024, 0.004, 0.032, M['coin'])
# legs stand upright in the world: the table frame is tilted 7 degrees
def leg(name, x, y, z, length):
    e = bpy.data.objects.new(name, None)
    coll.objects.link(e)
    e.location = (x, y, z)
    e.rotation_euler = (math.radians(-7), 0, 0)
    box(name + '_post', 0, 0, -length, 0.045, 0.045, length, M['steel'], parent=e)
    cylinder(name + '_foot', 0, 0, 0.018, -length - 0.012, -length, M['black'], seg=16, parent=e)
for name, x, y, length in (('leg_fl', cx0 + 0.03, cy0 + 0.03, 0.6), ('leg_fr', cx1 - 0.03, cy0 + 0.03, 0.6), ('leg_bl', cx0 + 0.03, cy1 - 0.03, 0.74), ('leg_br', cx1 - 0.03, cy1 - 0.03, 0.74)):
    leg(name, x, y, CABZ, length)

# backbox (upright in the world)
bb = bpy.data.objects.new('backbox', None)
coll.objects.link(bb)
bb.location = (0.019, cy1 + 0.01, 0.0)
bb.rotation_euler = (math.radians(-7), 0, 0)
W = cx1 - cx0 + 0.02
box('backbox_neck', 0, 0.03, -0.02, W * 0.6, 0.06, 0.06, M['cabinet'], parent=bb)
box('backbox_speaker_panel', 0, 0.05, 0.03, W, 0.11, 0.21, M['cabinet'], parent=bb)
box('backbox_shell', 0, 0.06, 0.24, W, 0.13, 0.42, M['cabinet'], parent=bb)
box('backbox_top', 0, 0.06, 0.66, W + 0.01, 0.14, 0.012, M['steel'], parent=bb)
quad_uv('backglass', [(-0.265, -0.0055, 0.26), (0.265, -0.0055, 0.26), (0.265, -0.0055, 0.645), (-0.265, -0.0055, 0.645)], M['backglass'], parent=bb)
quad_uv('dmd', [(-0.13, -0.0055, 0.075), (0.13, -0.0055, 0.075), (0.13, -0.0055, 0.205), (-0.13, -0.0055, 0.205)], M['dmd'], parent=bb)
box('dmd_frame', 0, -0.003, 0.068, 0.285, 0.004, 0.144, M['black'], parent=bb)
for k, x in enumerate((-0.2, 0.2)):
    sp = cylinder('speaker_%d' % k, 0, 0, 0.042, 0, 0.004, M['grille'], seg=28)
    sp.location = (x, -0.004, 0.14)
    sp.rotation_euler = (math.radians(90), 0, 0)
    sp.parent = bb
box('backbox_accent', 0, -0.005, 0.652, W - 0.02, 0.003, 0.003, M['accent'], parent=bb)

# ================================================================ export
os.makedirs(os.path.join(ROOT, 'assets'), exist_ok=True)
bpy.ops.wm.save_as_mainfile(filepath=os.path.join(ROOT, 'assets', 'table.blend'))
bpy.ops.export_scene.gltf(
    filepath=os.path.join(ROOT, 'assets', 'table.glb'),
    export_format='GLB',
    export_apply=True,
    export_yup=True,
    export_lights=False,
    export_cameras=False,
)
print('EXPORTED table.glb with', len(bpy.data.objects), 'objects')
