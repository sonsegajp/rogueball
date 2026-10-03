# Mesh helpers for the procedural models (Blender bpy + bmesh).
import bpy, bmesh, math

def coll():
    return bpy.context.scene.collection

MATS = {}
def mat(name, color, metal=0.0, rough=0.5, emit=None, strength=0.0, alpha=1.0):
    if name in MATS:
        return MATS[name]
    m = bpy.data.materials.new(name)
    try:
        m.use_nodes = True
    except Exception:
        pass
    p = next((n for n in m.node_tree.nodes if n.type == 'BSDF_PRINCIPLED'), None)
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
    MATS[name] = m
    return m

def srgb(h):
    """'#rrggbb' → linear rgb tuple for Blender colour inputs"""
    h = h.lstrip('#')
    c = [int(h[i:i + 2], 16) / 255 for i in (0, 2, 4)]
    return tuple(v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4 for v in c)

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
    coll().objects.link(ob)
    if m:
        me.materials.append(m)
    ob.location = loc
    if parent:
        ob.parent = parent
    return ob

def empty(name, loc=(0, 0, 0), rot=(0, 0, 0), parent=None):
    e = bpy.data.objects.new(name, None)
    coll().objects.link(e)
    e.location = loc
    e.rotation_euler = rot
    if parent:
        e.parent = parent
    return e

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

def flat(name, poly, z, m, **kw):
    """single-sided decal polygon facing up"""
    poly = [tuple(p[:2]) for p in poly]
    if area(poly) < 0:
        poly = poly[::-1]
    return make(name, [(x, y, z) for x, y in poly], [tuple(range(len(poly)))], m, recalc=False, **kw)

def sweep(name, pts, r, zb, zt, m, closed=False, round_top=False, **kw):
    """Rail swept along a 2D polyline; pts may carry per-point [x, y, zb, zt]."""
    n = len(pts)
    def d(i, j):
        dx = pts[j][0] - pts[i][0]; dy = pts[j][1] - pts[i][1]
        l = math.hypot(dx, dy) or 1
        return dx / l, dy / l
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
        rt = r if round_top else 0
        verts.append((x - mx * s, y - my * s, b0))
        verts.append((x + mx * s, y + my * s, b0))
        for cx, cz in crown:
            verts.append((x - mx * s * cx, y - my * s * cx, t0 - rt + cz * rt))
    ring = [0] + [2 + j for j in reversed(range(len(crown)))] + [1]
    faces = []
    for i in range(n if closed else n - 1):
        a = i * per; b = ((i + 1) % n) * per
        for j in range(len(ring)):
            p, q = ring[j], ring[(j + 1) % len(ring)]
            faces.append((a + p, b + p, b + q, a + q))
    if not closed:
        faces.append(tuple(ring[::-1]))
        faces.append(tuple((n - 1) * per + j for j in ring))
    return make(name, verts, faces, m, smooth=(lambda f: True) if round_top else None, **kw)

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

def sphere(name, x, y, z, r, m, seg=32, rings=16, **kw):
    verts, faces = [], []
    for j in range(rings + 1):
        t = math.pi * j / rings - math.pi / 2
        for i in range(seg):
            a = 2 * math.pi * i / seg
            verts.append((x + r * math.cos(t) * math.cos(a), y + r * math.cos(t) * math.sin(a), z + r * math.sin(t)))
    for j in range(rings):
        for i in range(seg):
            k = (i + 1) % seg
            faces.append((j * seg + i, j * seg + k, (j + 1) * seg + k, (j + 1) * seg + i))
    return make(name, verts, faces, m, smooth=True, **kw)

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

def circle_pts(cx, cy, r, seg=24, a0=0.0):
    return [(cx + math.cos(a0 + 2 * math.pi * i / seg) * r, cy + math.sin(a0 + 2 * math.pi * i / seg) * r) for i in range(seg)]

def circles_hull(centers, r, seg=20):
    pts = []
    for cx, cy in centers:
        pts += circle_pts(cx, cy, r, seg)
    return hull(pts)

def capsule_outline(length, r0, r1, seg=28):
    return hull(circle_pts(0, 0, r0, seg) + circle_pts(length, 0, r1, seg))

def nut(name, x, y, z, m, r=0.0032, h=0.0025, **kw):
    return cylinder(name, x, y, r, z, z + h, m, seg=6, **kw)

def text_mesh(name, body, size, x, y, z, m, font=None, rot=0.0, extrude=0.0003, **kw):
    cu = bpy.data.curves.new(name + '_c', 'FONT')
    cu.body = body
    cu.size = size
    cu.extrude = extrude
    cu.align_x = 'CENTER'
    cu.align_y = 'CENTER'
    if font:
        cu.font = font
    tmp = bpy.data.objects.new(name + '_tmp', cu)
    coll().objects.link(tmp)
    dg = bpy.context.evaluated_depsgraph_get()
    me = bpy.data.meshes.new_from_object(tmp.evaluated_get(dg))
    bpy.data.objects.remove(tmp)
    me.materials.clear()
    me.materials.append(m)
    ob = bpy.data.objects.new(name, me)
    coll().objects.link(ob)
    ob.location = (x, y, z)
    ob.rotation_euler = (0, 0, rot)
    if 'parent' in kw and kw['parent']:
        ob.parent = kw['parent']
    return ob

def load_font(*paths):
    import os
    for f in paths:
        if os.path.exists(f):
            try:
                return bpy.data.fonts.load(f)
            except Exception:
                pass
    return None

def clip_convex(subject, clip):
    """Sutherland–Hodgman: clip polygon `subject` to the convex CCW polygon `clip`."""
    def inside(p, a, b):
        return (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]) >= 0
    def inter(p, q, a, b):
        x1, y1, x2, y2 = p[0], p[1], q[0], q[1]
        x3, y3, x4, y4 = a[0], a[1], b[0], b[1]
        den = (x1 - x2) * (y3 - y4) - (y1 - y2) * (x3 - x4)
        if abs(den) < 1e-12:
            return q
        t = ((x1 - x3) * (y3 - y4) - (y1 - y3) * (x3 - x4)) / den
        return (x1 + t * (x2 - x1), y1 + t * (y2 - y1))
    if area(clip) < 0:
        clip = clip[::-1]
    out = list(subject)
    for i in range(len(clip)):
        a, b = clip[i], clip[(i + 1) % len(clip)]
        inp, out = out, []
        if not inp:
            break
        s = inp[-1]
        for e in inp:
            if inside(e, a, b):
                if not inside(s, a, b):
                    out.append(inter(s, e, a, b))
                out.append(e)
            elif inside(s, a, b):
                out.append(inter(s, e, a, b))
            s = e
    return out
