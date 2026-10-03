# Renders the parts kit for generated tables: every moving or reusable part, built at the origin and
# rendered from the game's camera into its own small sprite strip, at 1.0 and 0.8 px per mm.
#   blender --background --python art/kit.py              (everything)
#   blender --background --python art/kit.py -- bumper_*  (only matching sprites)
# Output: assets/kit/s100/*.png + kit.json, assets/kit/s80/*.png + kit.json
# kit.json gives each sprite's frame size and the offset of its top-left corner from the pixel the
# part's origin (x, y, 0) projects to, so the game can place it anywhere (the camera is orthographic).
import bpy, sys, os, json, math
import numpy as np
from mathutils import Vector

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import pixel
from geom import (mat, srgb, make, empty, prism, sweep, cylinder, dome, sphere, torus, tube, box,
                  circles_hull, capsule_outline, nut, load_font, text_mesh)

ROOT = os.path.dirname(HERE)
TMP = os.path.join(ROOT, 'build', 'kit_tmp')
os.makedirs(TMP, exist_ok=True)
ONLY = sys.argv[sys.argv.index('--') + 1:] if '--' in sys.argv else []

PHI = math.radians(20.0)
K = 4
R = 0.0135
SCALES = (('s100', 1000.0), ('s80', 800.0))

bpy.ops.wm.read_factory_settings(use_empty=True)
sc = bpy.context.scene
engines = [e.identifier for e in bpy.types.RenderSettings.bl_rna.properties['engine'].enum_items]
sc.render.engine = 'BLENDER_EEVEE_NEXT' if 'BLENDER_EEVEE_NEXT' in engines else 'BLENDER_EEVEE'
sc.render.film_transparent = True
sc.render.resolution_percentage = 100
sc.render.image_settings.file_format = 'PNG'
sc.render.image_settings.color_mode = 'RGBA'
sc.view_settings.view_transform = 'Standard'
sc.view_settings.look = 'None'
try:
    sc.eevee.taa_render_samples = 32
except Exception:
    pass

cam_data = bpy.data.cameras.new('cam')
cam_data.type = 'ORTHO'
cam_data.clip_end = 20
cam = bpy.data.objects.new('cam', cam_data)
sc.collection.objects.link(cam)
cam.rotation_euler = (PHI, 0, 0)
sc.camera = cam

# the same world and lights as the table renders
world = bpy.data.worlds.new('w')
sc.world = world
world.use_nodes = True
wn = world.node_tree
bg = wn.nodes.get('Background')
tc = wn.nodes.new('ShaderNodeTexCoord')
sep = wn.nodes.new('ShaderNodeSeparateXYZ')
ramp = wn.nodes.new('ShaderNodeValToRGB')
ramp.color_ramp.elements[0].position = 0.35
wn.links.new(tc.outputs['Generated'], sep.inputs[0])
wn.links.new(sep.outputs['Z'], ramp.inputs[0])
wn.links.new(ramp.outputs[0], bg.inputs[0])
bg.inputs[1].default_value = 1.0
def set_world(studio):
    ramp.color_ramp.elements[0].color = (0.32, 0.30, 0.40, 1) if studio else (0.04, 0.035, 0.07, 1)
    ramp.color_ramp.elements[1].color = (1.6, 1.55, 1.7, 1) if studio else (0.55, 0.52, 0.62, 1)

def light(name, kind, rot, energy, color, size):
    ld = bpy.data.lights.new(name, kind)
    ld.energy = energy
    ld.color = color
    ld.angle = size
    lo = bpy.data.objects.new(name, ld)
    lo.rotation_euler = rot
    sc.collection.objects.link(lo)
light('key', 'SUN', (math.radians(28), math.radians(-24), 0), 4.2, (1.0, 0.95, 0.88), math.radians(6))
light('fill', 'SUN', (math.radians(-35), math.radians(30), 0), 0.9, (0.6, 0.7, 1.0), math.radians(20))
light('rim', 'SUN', (math.radians(-70), 0, 0), 1.0, (1.0, 0.75, 0.55), math.radians(10))

# ID material for outlines: pass_index = object id * 16 + material ramp, 5 bits per channel
idm = bpy.data.materials.new('id')
idm.use_nodes = True
nt = idm.node_tree
for n in list(nt.nodes):
    nt.nodes.remove(n)
info = nt.nodes.new('ShaderNodeObjectInfo')
def math_node(op, a, b=None):
    n = nt.nodes.new('ShaderNodeMath')
    n.operation = op
    if isinstance(a, (int, float)):
        n.inputs[0].default_value = a
    else:
        nt.links.new(a, n.inputs[0])
    if b is not None:
        if isinstance(b, (int, float)):
            n.inputs[1].default_value = b
        else:
            nt.links.new(b, n.inputs[1])
    return n.outputs[0]
idx = info.outputs['Object Index']
rr = math_node('DIVIDE', math_node('MODULO', idx, 32), 31)
gg = math_node('DIVIDE', math_node('MODULO', math_node('FLOOR', math_node('DIVIDE', idx, 32)), 32), 31)
bb = math_node('DIVIDE', math_node('FLOOR', math_node('DIVIDE', idx, 1024)), 31)
comb = nt.nodes.new('ShaderNodeCombineColor')
nt.links.new(rr, comb.inputs[0]); nt.links.new(gg, comb.inputs[1]); nt.links.new(bb, comb.inputs[2])
em = nt.nodes.new('ShaderNodeEmission')
nt.links.new(comb.outputs[0], em.inputs[0])
out = nt.nodes.new('ShaderNodeOutputMaterial')
nt.links.new(em.outputs[0], out.inputs[0])

def material_ramp(o):
    if not o.data or not getattr(o.data, 'materials', None) or not o.data.materials:
        return 0
    m = o.data.materials[0]
    p = next((n for n in m.node_tree.nodes if n.type == 'BSDF_PRINCIPLED'), None) if m.node_tree else None
    if p is None:
        return 0
    col = list(p.inputs['Base Color'].default_value)[:3]
    if p.inputs['Emission Strength'].default_value > 1.0:
        col = list(p.inputs['Emission Color'].default_value)[:3]
    s = [c * 12.92 if c <= 0.0031308 else 1.055 * c ** (1 / 2.4) - 0.055 for c in col]
    if p.inputs['Metallic'].default_value > 0.5:
        return 0
    return pixel.nearest_ramp(s)

def read_png(path):
    img = bpy.data.images.load(path, check_existing=False)
    w, h = img.size
    a = np.empty(w * h * 4, np.float32)
    img.pixels.foreach_get(a)
    bpy.data.images.remove(img)
    return a.reshape(h, w, 4)[::-1].copy()

def save_png(path, rgba8):
    h, w = rgba8.shape[:2]
    img = bpy.data.images.new('out', w, h, alpha=True)
    img.pixels.foreach_set((rgba8[::-1].astype(np.float32) / 255).ravel())
    img.filepath_raw = path
    img.file_format = 'PNG'
    img.save()
    bpy.data.images.remove(img)

def render(name, ids=False):
    path = os.path.join(TMP, name + ('_id' if ids else '') + '.png')
    sc.render.filepath = path
    vl = bpy.context.view_layer
    if ids:
        vl.material_override = idm
        filt = sc.render.filter_size
        sc.render.filter_size = 0.01
    bpy.ops.render.render(write_still=True)
    if ids:
        vl.material_override = None
        sc.render.filter_size = filt
    px = read_png(path)
    return pixel.decode_ids(px) if ids else px

def project(x, y, z, ppm):
    """pixel offset from the projected origin"""
    return x * ppm, -(y * math.cos(PHI) + z * math.sin(PHI)) * ppm

# ------------------------------------------------------------------ materials (as in table_model.py)
def M(name, hexc, metal=0.0, rough=0.5, emit=None, strength=0.0):
    return mat(name, srgb(hexc), metal, rough, srgb(emit) if emit else None, strength)
m = {
    'steel': M('steel', '#9a99b3', metal=0.55, rough=0.4),
    'chrome': M('chrome', '#c7c7d9', metal=1.0, rough=0.15),
    'rubber': M('rubber', '#1b1830', rough=0.85),
    'rub_red': M('rubber_red', '#b8222f', rough=0.6),
    'flip': M('flipper_body', '#eef0f7', rough=0.35),
    'skirt': M('bumper_skirt', '#b8222f', rough=0.4),
    'b_body': M('bumper_body', '#c7c7d9', rough=0.3),
    'b_base': M('bumper_base', '#0b0a14', rough=0.5),
    'stripe': M('stripe', '#eef0f7', rough=0.4),
    'black': M('black', '#0b0a14', metal=0.3, rough=0.5),
    'wire': M('wire', '#c7c7d9', metal=1.0, rough=0.2),
    'pink': M('pf_pink', '#a81c66', rough=0.4),
    'plastic_L': M('plastic', '#e0337f', rough=0.25),
    'plastic_R': M('plastic_c', '#1c95c8', rough=0.25),
    'gi': M('gi', '#fff1a8', emit='#ffcc4d', strength=6.0),
}
SUIT = load_font('C:/Windows/Fonts/seguisym.ttf')

def star_post(name, x, y, z0, z1, mm, ro=0.0034, ri=0.0024, n=8):
    pts = []
    for i in range(n * 2):
        a = math.pi * i / n
        r = ro if i % 2 == 0 else ri
        pts.append((x + math.cos(a) * r, y + math.sin(a) * r))
    return prism(name, pts, z0, z1, mm)

def screw(name, x, y, z, mm):
    return dome(name, x, y, 0.0024, z, 0.0013, mm, seg=12, rings=3)

# ------------------------------------------------------------------ parts: each returns {objs, frames, pose, ...}
def none(i):
    pass

def part_ball():
    ballm = mat('ball_chrome', srgb('#eef0f7'), metal=1.0, rough=0.16)
    return {'objs': [sphere('ball', 0, 0, R, R, ballm, seg=48, rings=24)], 'frames': 1, 'pose': none, 'studio': True, 'origin_z': R}

def part_flipper(side, short):
    length, r0, r1 = (0.055, 0.011, 0.006) if short else (0.074, 0.0125, 0.0065)
    rest, up = ((-27.0, 30.0) if side == 'L' else (207.0, 150.0))
    h = 0.024
    piv = empty('flip_piv', loc=(0, 0, 0))
    parts = [
        prism('f_body', capsule_outline(length, r0 - 0.003, r1 - 0.003), 0.002, h, m['flip'], parent=piv),
        prism('f_rubber', capsule_outline(length, r0, r1), 0.007, 0.019, m['rub_red'], parent=piv),
        cylinder('f_cap', 0, 0, 0.0065, h, h + 0.002, m['chrome'], seg=24, parent=piv),
        nut('f_bolt', 0, 0, h + 0.002, m['steel'], r=0.003, parent=piv),
        prism('f_stripe', [(0.016, -0.0022), (length - 0.004, -0.0012), (length - 0.004, 0.0012), (0.016, 0.0022)], h, h + 0.0004, m['pink'], parent=piv),
    ]
    a0, a1 = math.radians(rest), math.radians(up)
    def pose(i):
        piv.rotation_euler = (0, 0, a0 + (a1 - a0) * i / 11)
    return {'objs': parts, 'frames': 12, 'pose': pose}

def part_bumper(color):
    caps = ['#49c6ec', '#ff6aa8', '#ffcc4d'][color]
    dark = ['#12669a', '#a81c66', '#b8650f'][color]
    r = 0.03
    objs = [cylinder('b_base', 0, 0, r - 0.001, 0, 0.003, m['b_base']),
            cylinder('b_skirt', 0, 0, r, 0.003, 0.009, m['skirt'], r_top=r - 0.006),
            cylinder('b_body', 0, 0, r * 0.72, 0.009, 0.033, m['b_body'])]
    for k in range(3):
        a = 2 * math.pi * k / 3 + 0.5
        objs.append(cylinder('b_rod%d' % k, math.cos(a) * r * 0.82, math.sin(a) * r * 0.82, 0.0012, 0.009, 0.033, m['steel'], seg=8))
    ring = cylinder('b_ring', 0, 0, r + 0.0005, 0.015, 0.0195, m['chrome'])
    capm = mat('bumper_cap_%d' % color, srgb(dark), rough=0.25, emit=srgb(caps), strength=0.0)
    objs += [ring, cylinder('b_cap', 0, 0, r + 0.002, 0.033, 0.038, capm), dome('b_dome', 0, 0, r * 0.72, 0.038, 0.006, capm),
             cylinder('b_logo', 0, 0, r * 0.36, 0.0442, 0.0446, m['stripe'], seg=5)]
    def pose(i):
        ring.location.z = -(0.0, 0.004, 0.008, 0.011)[i % 4]
        p = next(n for n in capm.node_tree.nodes if n.type == 'BSDF_PRINCIPLED')
        p.inputs['Emission Strength'].default_value = 9.0 if i >= 4 else 0.0
    return {'objs': objs, 'frames': 8, 'pose': pose}

def sling_pts(side):
    s = -1 if side == 'L' else 1
    # local to the bottom-outer corner (pts[1]); the game places the sprite there
    return [(0.0, 0.09), (0.0, 0.0), (-s * 0.052, -0.03)]

def part_sling(side):
    pts = sling_pts(side)
    r = 0.0055
    a, c = pts[0], pts[2]
    gx = sum(p[0] for p in pts) / 3; gy = sum(p[1] for p in pts) / 3
    nx, ny = -(c[1] - a[1]), c[0] - a[0]
    nl = math.hypot(nx, ny); nx /= nl; ny /= nl
    if nx * ((a[0] + c[0]) / 2 - gx) + ny * ((a[1] + c[1]) / 2 - gy) < 0:
        nx, ny = -nx, -ny
    objs = []
    for j, p in enumerate(pts):
        objs.append(cylinder('s_post%d' % j, p[0], p[1], 0.0028, 0, 0.046, m['chrome'], seg=12))
        objs.append(nut('s_nut%d' % j, p[0], p[1], 0.046, m['chrome']))
    objs.append(dome('s_bulb', gx, gy, 0.004, 0.0, 0.009, m['gi'], seg=12, rings=4))
    bands, arms = [], []
    for kf, bow in enumerate((0.0, 0.003, 0.006)):
        mid = ((a[0] + c[0]) / 2 + nx * bow, (a[1] + c[1]) / 2 + ny * bow)
        hullpts = circles_hull(pts + [mid], r - 0.0018, seg=24)
        bands.append(sweep('s_band%d' % kf, hullpts, 0.0018, R - 0.0055, R + 0.0055, m['rubber'], closed=True))
        ang = math.atan2(c[1] - a[1], c[0] - a[0])
        arms.append(box('s_arm%d' % kf, (a[0] + c[0]) / 2 - nx * (0.006 - bow), (a[1] + c[1]) / 2 - ny * (0.006 - bow), 0.004, 0.03, 0.004, 0.012, m['black'], rot=ang))
    glow = mat('sling_glow_' + side, srgb('#ff6aa8'), emit=srgb('#ff6aa8'), strength=0.4)
    strip = sweep('s_strip', [(a[0] + nx * (r + 0.004), a[1] + ny * (r + 0.004)), (c[0] + nx * (r + 0.004), c[1] + ny * (r + 0.004))], 0.0014, 0.0005, 0.0012, glow)
    def pose(i):
        kf, lit = i % 3, i // 3
        for k in range(3):
            bands[k].hide_render = k != kf
            arms[k].hide_render = k != kf
        p = next(n for n in glow.node_tree.nodes if n.type == 'BSDF_PRINCIPLED')
        p.inputs['Emission Strength'].default_value = 8.0 if lit else 0.4
    return {'objs': objs + bands + arms + [strip], 'frames': 6, 'pose': pose}

def part_sling_plastic(side, lit):
    pts = sling_pts(side)
    gx = sum(p[0] for p in pts) / 3; gy = sum(p[1] for p in pts) / 3
    grown = circles_hull(pts, 0.0055 + 0.006, seg=16)
    if lit:
        litm = mat('plastic_lit_' + side, srgb('#ffb3d2' if side == 'L' else '#a3ecff'), emit=srgb('#ffb3d2' if side == 'L' else '#a3ecff'), strength=4.0)
        return {'objs': [prism('sp_lit', grown, 0.0486, 0.0487, litm)], 'frames': 1, 'pose': none, 'outline': False}
    objs = [prism('sp', grown, 0.046, 0.0485, m['plastic_' + side]),
            text_mesh('sp_sym', '♠' if side == 'L' else '♥', 0.024, gx, gy + 0.004, 0.0487, m['stripe'], font=SUIT)]
    for j, p in enumerate(pts):
        objs.append(screw('sp_screw%d' % j, p[0], p[1], 0.0485, m['chrome']))
    return {'objs': objs, 'frames': 1, 'pose': none}

def part_drop(deg):
    rot = math.radians(deg)
    w, r, zt = 0.029, 0.004, 0.035
    slot = box('d_slot', 0, 0, 0.0001, w + 0.002, 2 * r + 0.003, 0.0002, m['black'], rot=rot)
    piv = empty('d_piv', loc=(0, 0, 0))
    dm = mat('drop_face', srgb('#e89a1c'), rough=0.35)
    t = box('d_face', 0, 0, 0, w - 0.002, 2 * r - 0.001, zt, dm, rot=rot, parent=piv)
    st = box('d_stripe', math.sin(rot) * r, -math.cos(rot) * r, 0.013, w * 0.7, 0.0008, 0.006, m['stripe'], rot=rot, parent=piv)
    def pose(i):
        piv.location.z = -zt * i / 5
    return {'objs': [slot, t, st], 'frames': 6, 'pose': pose}

def part_standup(deg):
    rot = math.radians(deg)
    w, r, zt = 0.026, 0.004, 0.035
    piv = empty('st_piv', loc=(0, 0, 0), rot=(0, 0, rot))
    sm = mat('stand_face', srgb('#e0337f'), rough=0.35)
    t = box('st_face', 0, 0, 0.006, w, 2 * r - 0.002, zt - 0.006, sm, parent=piv)
    st = box('st_stripe', 0, -r * 0.97, 0.016, w * 0.6, 0.0008, 0.005, m['stripe'], parent=piv)
    post = cylinder('st_post', 0, 0.0005, 0.0022, 0, 0.007, m['black'], seg=10, parent=piv)
    def pose(i):
        piv.rotation_euler = ((-0.24, -0.12, 0.0, 0.12, 0.24)[i], 0, rot)
    return {'objs': [t, st, post], 'frames': 5, 'pose': pose}

def part_spinner():
    z0 = 0.042
    half = 0.0225
    piv = empty('sp_piv', loc=(0, 0, z0))
    w = 2 * half - 0.005
    plate = box('sp_plate', 0, 0, -0.022, w, 0.0016, 0.02, m['chrome'], parent=piv)
    decm = mat('spinner_decal', srgb('#e0337f'), rough=0.4)
    dec = box('sp_decal', 0, -0.0009, -0.019, w * 0.8, 0.0004, 0.014, decm, parent=piv)
    objs = [plate, dec]
    for k, px in enumerate((-half, half)):
        objs.append(tube('sp_bracket%d' % k, [(px, 0, 0), (px, 0, z0 + 0.004), (px + (0.002 if k == 0 else -0.002), 0, z0 + 0.004)], 0.0013, m['wire']))
    objs.append(tube('sp_axle', [(-half, 0, z0), (half, 0, z0)], 0.001, m['steel']))
    def pose(i):
        piv.rotation_euler = (2 * math.pi * i / 12, 0, 0)
    return {'objs': objs, 'frames': 12, 'pose': pose}

def part_gate():
    half = 0.015
    piv = empty('g_piv', loc=(0, 0, 0.04))
    flap = tube('g_wire', [(-half + 0.002, 0, 0), (-half + 0.002, 0, -0.026), (half - 0.002, 0, -0.026), (half - 0.002, 0, 0)], 0.0011, m['wire'], seg=6, parent=piv)
    objs = [flap]
    for k, px in enumerate((-half, half)):
        objs.append(tube('g_bracket%d' % k, [(px, 0, 0), (px, 0, 0.041)], 0.0013, m['wire']))
    objs.append(tube('g_axle', [(-half, 0, 0.04), (half, 0, 0.04)], 0.001, m['steel']))
    def pose(i):
        piv.rotation_euler = (-1.2 * i / 4, 0, 0)
    return {'objs': objs, 'frames': 5, 'pose': pose}

def part_plunger():
    y0 = 0.0465
    piv = empty('pl_piv', loc=(0, 0, R))
    def cyl_y(name, ya, yb, r, mm):
        seg = 16
        verts, faces = [], []
        for i in range(seg):
            a = 2 * math.pi * i / seg
            verts += [(math.cos(a) * r, ya, math.sin(a) * r), (math.cos(a) * r, yb, math.sin(a) * r)]
        for i in range(seg):
            j = (i + 1) % seg
            faces.append((i * 2, j * 2, j * 2 + 1, i * 2 + 1))
        faces.append(tuple(i * 2 for i in range(seg)))
        faces.append(tuple(i * 2 + 1 for i in range(seg - 1, -1, -1)))
        return make(name, verts, faces, mm, smooth=lambda k: k < seg, parent=piv)
    parts = [cyl_y('pl_rod', -0.03, y0 - 0.004, 0.003, m['chrome']), cyl_y('pl_tip', y0 - 0.007, y0, 0.0085, m['rubber'])]
    coil = [(0.0058 * math.cos(t * 0.8), -0.028 + t * 0.0011, 0.0058 * math.sin(t * 0.8)) for t in range(0, 60)]
    parts.append(tube('pl_spring', coil, 0.0009, m['steel'], seg=6, parent=piv))
    def pose(i):
        piv.location.y = -0.045 * i / 7
    return {'objs': parts, 'frames': 8, 'pose': pose}

def part_post(kind):
    if kind == 'rubber':
        objs = [star_post('p_star', 0, 0, 0, 0.045, m['chrome']), torus('p_ring', 0, 0, R, 0.0055 - 0.0028, 0.0028, m['rubber']),
                nut('p_nut', 0, 0, 0.045, m['chrome'])]
    else:
        r, zt = (0.006, 0.055) if kind == 'metal_l' else (0.004, 0.045)
        objs = [cylinder('p_post', 0, 0, r, 0, zt, m['chrome'], seg=24), nut('p_nut', 0, 0, zt, m['chrome'], r=r * 0.7)]
    return {'objs': objs, 'frames': 1, 'pose': none}

def part_rollover():
    objs = [box('ro_slot', 0, 0, 0.0001, 0.004, 0.026, 0.0002, m['black']),
            tube('ro_wire', [(0, -0.012, 0.0), (0, -0.006, 0.004), (0, 0, 0.0055), (0, 0.006, 0.004), (0, 0.012, 0.0)], 0.0012, m['wire'], seg=6)]
    return {'objs': objs, 'frames': 1, 'pose': none, 'outline': False}

PARTS = {
    'ball': part_ball,
    'flipper_L': lambda: part_flipper('L', False), 'flipper_R': lambda: part_flipper('R', False),
    'flipperS_L': lambda: part_flipper('L', True), 'flipperS_R': lambda: part_flipper('R', True),
    'bumper_0': lambda: part_bumper(0), 'bumper_1': lambda: part_bumper(1), 'bumper_2': lambda: part_bumper(2),
    'sling_L': lambda: part_sling('L'), 'sling_R': lambda: part_sling('R'),
    'slingplastic_L': lambda: part_sling_plastic('L', False), 'slingplastic_R': lambda: part_sling_plastic('R', False),
    'slinglit_L': lambda: part_sling_plastic('L', True), 'slinglit_R': lambda: part_sling_plastic('R', True),
    'spinner': part_spinner, 'gate': part_gate, 'plunger': part_plunger,
    'post_rubber': lambda: part_post('rubber'), 'post_metal': lambda: part_post('metal'), 'post_metal_l': lambda: part_post('metal_l'),
    'rollover': part_rollover,
}
for d in range(-30, 31, 10):
    PARTS['drop_%d' % d] = (lambda d=d: part_drop(d))
for d in range(0, 360, 15):
    PARTS['stand_%d' % d] = (lambda d=d: part_standup(d))

def want(name):
    return not ONLY or any(name == p or (p.endswith('*') and name.startswith(p[:-1])) for p in ONLY)

def clear_scene():
    for o in list(bpy.data.objects):
        if o.type in ('MESH', 'EMPTY', 'CURVE', 'FONT'):
            bpy.data.objects.remove(o, do_unlink=True)

def bbox(spr, ppm):
    xs, ys = [], []
    for i in range(spr['frames']):
        spr['pose'](i)
        bpy.context.view_layer.update()
        for o in spr['objs']:
            if o.type != 'MESH' or o.hide_render:
                continue
            for c in o.bound_box:
                w = o.matrix_world @ Vector(c)
                px, py = project(w.x, w.y, w.z, ppm)
                xs.append(px); ys.append(py)
    for o in spr['objs']:
        o.hide_render = False
    mg = 3
    return int(math.floor(min(xs))) - mg, int(math.ceil(max(xs))) + mg, int(math.floor(min(ys))) - mg, int(math.ceil(max(ys))) + mg

def frame_camera(box, ppm):
    x0, x1, y0, y1 = box
    w, h = x1 - x0, y1 - y0
    sc.render.resolution_x = w * K
    sc.render.resolution_y = h * K
    cam_data.ortho_scale = max(w, h) / ppm
    cx = (x0 + x1) / 2 / ppm
    syc = -(y0 + y1) / 2 / ppm
    cam.location = (cx, syc * math.cos(PHI) - 4 * math.sin(PHI), syc * math.sin(PHI) + 4 * math.cos(PHI))
    return w, h

for tag, ppm in SCALES:
    out_dir = os.path.join(ROOT, 'assets', 'kit', tag)
    os.makedirs(out_dir, exist_ok=True)
    jpath = os.path.join(out_dir, 'kit.json')
    kit = json.load(open(jpath)) if (ONLY and os.path.exists(jpath)) else {}
    for name, build in PARTS.items():
        if not want(name):
            continue
        clear_scene()
        spr = build()
        objs = [o for o in bpy.data.objects if o.type in ('MESH', 'CURVE', 'FONT')]
        for i, o in enumerate(objs):
            o.pass_index = (i + 2) * 16 + material_ramp(o)
        set_world(spr.get('studio', False))
        oz = spr.get('origin_z', 0.0)
        box_ = bbox(spr, ppm)
        # the ball is placed by its centre, everything else by its floor point
        ozx, ozy = project(0, 0, oz, ppm)
        w, h = frame_camera(box_, ppm)
        frames = []
        for i in range(spr['frames']):
            spr['pose'](i)
            bpy.context.view_layer.update()
            rgba = render('%s_%s_%d' % (tag, name, i))
            ids, ramps = render('%s_%s_%d' % (tag, name, i), ids=True)
            outline = spr.get('outline', True)
            art = pixel.pixelize(rgba, K, ids=ids if outline else None, ramps=ramps, alpha='hard',
                                 outer_outline=outline, inner_outline=outline)
            frames.append(art[:h, :w])
        strip = np.concatenate(frames, axis=1)
        save_png(os.path.join(out_dir, name + '.png'), strip)
        kit[name] = {'file': name + '.png', 'frames': spr['frames'], 'w': w, 'h': h, 'ox': box_[0] - ozx, 'oy': box_[2] - ozy}
        print('KIT', tag, name, w, h, spr['frames'], flush=True)
    json.dump(kit, open(jpath, 'w'), indent=1)
print('DONE', flush=True)
