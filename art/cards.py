# Unique pixel-art illustrations for every item and card: a small modeled scene per id, rendered and pixelized.
#   blender --background --python art/cards.py            (all)
#   blender --background --python art/cards.py -- jester  (some)
# Output: assets/cards/<id>.png (48×48)
import bpy, sys, os, math
import numpy as np
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import pixel
from geom import mat, srgb, make, prism, sweep, cylinder, dome, sphere, torus, tube, box, circle_pts, load_font, text_mesh

ROOT = os.path.dirname(HERE)
OUT = os.path.join(ROOT, 'assets', 'cards')
TMP = os.path.join(ROOT, 'build', 'card_tmp')
os.makedirs(OUT, exist_ok=True)
os.makedirs(TMP, exist_ok=True)
ONLY = sys.argv[sys.argv.index('--') + 1:] if '--' in sys.argv else []
SIZE, K = 48, 4

bpy.ops.wm.read_factory_settings(use_empty=True)
sc = bpy.context.scene
engines = [e.identifier for e in bpy.types.RenderSettings.bl_rna.properties['engine'].enum_items]
sc.render.engine = 'BLENDER_EEVEE_NEXT' if 'BLENDER_EEVEE_NEXT' in engines else 'BLENDER_EEVEE'
sc.render.resolution_x = sc.render.resolution_y = SIZE * K
sc.render.film_transparent = True
sc.render.image_settings.file_format = 'PNG'
sc.render.image_settings.color_mode = 'RGBA'
sc.view_settings.view_transform = 'Standard'
try:
    sc.eevee.taa_render_samples = 32
except Exception:
    pass

FONT_SYM = load_font('C:/Windows/Fonts/seguisym.ttf')
FONT_BOLD = load_font('C:/Windows/Fonts/bahnschrift.ttf', 'C:/Windows/Fonts/impact.ttf')

# ------------------------------------------------------------------ fixed rig: camera, lights, world
cam_data = bpy.data.cameras.new('cam')
cam_data.type = 'ORTHO'
cam_data.ortho_scale = 2.7
cam = bpy.data.objects.new('cam', cam_data)
sc.collection.objects.link(cam)
yaw, pitch, dist = math.radians(28), math.radians(58), 10
cam.location = (dist * math.sin(pitch) * math.sin(yaw), -dist * math.sin(pitch) * math.cos(yaw), dist * math.cos(pitch) + 0.25)
cam.rotation_euler = (pitch, 0, yaw)
sc.camera = cam
world = bpy.data.worlds.new('w')
sc.world = world
world.use_nodes = True
# studio gradient: bright above, dark below, so chrome and gold read as metal
wn = world.node_tree
bgn = wn.nodes['Background']
tc = wn.nodes.new('ShaderNodeTexCoord')
sep = wn.nodes.new('ShaderNodeSeparateXYZ')
cr = wn.nodes.new('ShaderNodeValToRGB')
cr.color_ramp.elements[0].color = (0.03, 0.025, 0.06, 1)
cr.color_ramp.elements[1].color = (1.0, 0.97, 0.92, 1)
cr.color_ramp.elements[0].position = 0.42
cr.color_ramp.elements[1].position = 0.75
wn.links.new(tc.outputs['Generated'], sep.inputs[0])
wn.links.new(sep.outputs['Z'], cr.inputs[0])
wn.links.new(cr.outputs[0], bgn.inputs[0])
bgn.inputs[1].default_value = 1.0
RIG = {'cam'}
def light(name, rot, energy, color, angle):
    ld = bpy.data.lights.new(name, 'SUN')
    ld.energy = energy; ld.color = color; ld.angle = angle
    lo = bpy.data.objects.new(name, ld)
    lo.rotation_euler = rot
    sc.collection.objects.link(lo)
    RIG.add(name)
light('key', (math.radians(40), math.radians(-30), math.radians(-20)), 4.0, (1.0, 0.95, 0.88), math.radians(5))
light('rim', (math.radians(-60), math.radians(20), math.radians(160)), 2.5, (0.7, 0.8, 1.0), math.radians(8))
light('fill', (math.radians(70), math.radians(40), math.radians(60)), 0.6, (1.0, 0.8, 0.7), math.radians(20))

# ID material for outlines (same encoding as the table renderer)
idm = bpy.data.materials.new('id')
idm.use_nodes = True
nt = idm.node_tree
for n in list(nt.nodes):
    nt.nodes.remove(n)
info = nt.nodes.new('ShaderNodeObjectInfo')
def mn(op, a, b=None):
    n = nt.nodes.new('ShaderNodeMath'); n.operation = op
    if isinstance(a, (int, float)): n.inputs[0].default_value = a
    else: nt.links.new(a, n.inputs[0])
    if b is not None:
        if isinstance(b, (int, float)): n.inputs[1].default_value = b
        else: nt.links.new(b, n.inputs[1])
    return n.outputs[0]
ix = info.outputs['Object Index']
rr = mn('DIVIDE', mn('MODULO', ix, 32), 31)
gg = mn('DIVIDE', mn('MODULO', mn('FLOOR', mn('DIVIDE', ix, 32)), 32), 31)
bb = mn('DIVIDE', mn('FLOOR', mn('DIVIDE', ix, 1024)), 31)
cmb = nt.nodes.new('ShaderNodeCombineColor')
nt.links.new(rr, cmb.inputs[0]); nt.links.new(gg, cmb.inputs[1]); nt.links.new(bb, cmb.inputs[2])
em = nt.nodes.new('ShaderNodeEmission'); nt.links.new(cmb.outputs[0], em.inputs[0])
out = nt.nodes.new('ShaderNodeOutputMaterial'); nt.links.new(em.outputs[0], out.inputs[0])

# ------------------------------------------------------------------ materials
def M(name, hexc, metal=0.0, rough=0.45, emit=None, strength=0.0):
    return mat('c_' + name, srgb(hexc), metal, rough, srgb(emit) if emit else None, strength)
P = {
    'gold': M('gold', '#e89a1c', metal=1.0, rough=0.25), 'gold_l': M('gold_l', '#ffcc4d', metal=1.0, rough=0.2),
    'chrome': M('chrome', '#c7c7d9', metal=1.0, rough=0.12), 'steel': M('steel', '#9a99b3', metal=0.8, rough=0.35),
    'dark': M('dark', '#2e2b47'), 'black': M('black', '#1b1830', rough=0.6), 'white': M('white', '#eef0f7'),
    'pink': M('pink', '#e0337f'), 'pink_l': M('pink_l', '#ff6aa8'), 'cyan': M('cyan', '#1c95c8'), 'cyan_l': M('cyan_l', '#49c6ec'),
    'red': M('red', '#b8222f'), 'red_l': M('red_l', '#ee4040'), 'green': M('green', '#1f8a4c'), 'green_l': M('green_l', '#3fc46a'),
    'violet': M('violet', '#5b45b0'), 'violet_l': M('violet_l', '#7d68d4'), 'orange': M('orange', '#e89a1c'),
    'wood': M('wood', '#7a4524'), 'wood_l': M('wood_l', '#a8693a'), 'cream': M('cream', '#d39a62'),
    'glass': M('glass', '#a3ecff', rough=0.05), 'glow': M('glow', '#fff1a8', emit='#ffcc4d', strength=4.0),
    'fire': M('fire', '#ffcc4d', emit='#e89a1c', strength=3.0), 'fire_r': M('fire_r', '#ee4040', emit='#b8222f', strength=2.0),
    'paper': M('paper', '#eef0f7', rough=0.7), 'blueprint': M('blueprint', '#12669a', rough=0.7),
}

# ------------------------------------------------------------------ prop kit (unit scale, z up, centred on the origin)
def lathe(name, prof, m, seg=28, z0=0.0, x=0.0, y=0.0):
    """revolve [(r, z), ...] around the z axis"""
    verts, faces = [], []
    n = len(prof)
    for i in range(seg):
        a = 2 * math.pi * i / seg
        for r, z in prof:
            verts.append((x + r * math.cos(a), y + r * math.sin(a), z0 + z))
    for i in range(seg):
        i2 = (i + 1) % seg
        for j in range(n - 1):
            faces.append((i * n + j, i2 * n + j, i2 * n + j + 1, i * n + j + 1))
    return make(name, verts, faces, m, smooth=True)

def rot(o, x=0.0, y=0.0, z=0.0):
    o.rotation_euler = (math.radians(x), math.radians(y), math.radians(z)); return o
def at(o, x=0.0, y=0.0, z=0.0):
    o.location = (x, y, z); return o
def scl(o, x=1.0, y=None, z=None):
    o.scale = (x, x if y is None else y, x if z is None else z); return o

def glyph(name, ch, size, m, font=None, depth=0.08, x=0.0, y=0.0, z=0.0, upright=True):
    t = text_mesh(name, ch, size, x, y, z, m, font=font or FONT_SYM, extrude=depth)
    if upright:
        t.rotation_euler = (math.radians(90), 0, 0)
    return t

def coin(name, x=0.0, y=0.0, z=0.0, r=0.4, m=None, tilt=0.0):
    c = cylinder(name, 0, 0, r, -0.05, 0.05, m or P['gold'], seg=28)
    rim = torus(name + '_rim', 0, 0, 0.0, r - 0.02, 0.04, m or P['gold'], seg=28, rseg=6)
    rim.parent = c
    at(c, x, y, z); rot(c, tilt, 0, 0)
    return c
def chrome_ball(name, x=0.0, y=0.0, z=0.0, r=0.4):
    return sphere(name, x, y, z, r, P['chrome'], seg=32, rings=16)
def pip_cube(name, x, y, z, s, pips, m_body, m_pip):
    b = box(name, 0, 0, -s / 2, s, s, s, m_body)
    at(b, x, y, z)
    offs = {1: [(0, 0)], 2: [(-1, -1), (1, 1)], 3: [(-1, -1), (0, 0), (1, 1)], 4: [(-1, -1), (1, -1), (-1, 1), (1, 1)], 5: [(-1, -1), (1, -1), (0, 0), (-1, 1), (1, 1)], 6: [(-1, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (1, 1)]}[pips]
    for k, (u, v) in enumerate(offs):
        p = sphere('%s_p%d' % (name, k), u * s * 0.27, v * s * 0.27, s / 2, s * 0.09, m_pip, seg=10, rings=6)
        p.parent = b
    return b
def card_prop(name, sym, m_sym, x=0.0, y=0.0, z=0.0, rz=0.0, tilt=70):
    c = box(name, 0, 0, 0, 0.9, 1.25, 0.04, P['white'])
    s = glyph(name + '_s', sym, 0.7, m_sym, depth=0.02, z=0.05, upright=False)
    s.parent = c
    at(c, x, y, z); rot(c, tilt, 0, rz)
    return c
def flipper_prop(name, m=None):
    from geom import capsule_outline
    f = prism(name, capsule_outline(1.4, 0.32, 0.16), -0.15, 0.15, P['white'])
    r = prism(name + '_r', capsule_outline(1.4, 0.35, 0.19), -0.07, 0.07, m or P['red'])
    r.parent = f
    return f
def bolt(name, m=None, s=1.0):
    pts = [(0.15, 0.8), (-0.35, 0.0), (-0.02, 0.0), (-0.2, -0.8), (0.35, 0.1), (0.02, 0.1)]
    b = prism(name, [(x * s, y * s) for x, y in pts], -0.08, 0.08, m or P['fire'])
    return rot(b, 90, 0, 0)
def arrow_up(name, m, s=1.0):
    pts = [(-0.15, -0.6), (0.15, -0.6), (0.15, 0.1), (0.4, 0.1), (0.0, 0.65), (-0.4, 0.1), (-0.15, 0.1)]
    a = prism(name, [(x * s, y * s) for x, y in pts], -0.08, 0.08, m)
    return rot(a, 90, 0, 0)
def plate(name, x, y, z, w, d, h, m):
    return box(name, x, y, z, w, d, h, m)

# ------------------------------------------------------------------ recipes: one per item / card id
R = {}
def recipe(*ids):
    def deco(f):
        for i in ids:
            R[i] = f
        return f
    return deco

@recipe('jester')
def _():
    for k, (ang, m) in enumerate([(-40, P['pink']), (0, P['cyan']), (40, P['gold_l'])]):
        c = cylinder('horn%d' % k, 0, 0, 0.28, 0, 1.0, m, seg=20, r_top=0.03)
        rot(at(c, 0, 0, -0.3), 0, ang, 0)
        a = math.radians(ang)
        sphere('bell%d' % k, math.sin(a) * 1.05, 0, -0.3 + math.cos(a) * 1.05, 0.13, P['gold'])
    torus('band', 0, 0, -0.35, 0.36, 0.09, P['violet'])

@recipe('bumper_cars')
def _():
    body = box('car', 0, 0, -0.35, 1.3, 0.85, 0.45, P['cyan'])
    torus('fender', 0, 0, -0.32, 0.72, 0.1, P['black'])
    for k, (x, y) in enumerate([(-0.45, -0.4), (0.45, -0.4), (-0.45, 0.4), (0.45, 0.4)]):
        rot(at(cylinder('w%d' % k, 0, 0, 0.16, -0.06, 0.06, P['black']), x, y, -0.35), 90, 0, 0)
    tube('pole', [(0, 0.2, 0.1), (0, 0.2, 1.0)], 0.04, P['steel'])
    sphere('spark', 0, 0.2, 1.05, 0.1, P['glow'])
    dome('seat', 0, -0.1, 0.3, 0.1, 0.25, P['pink'])

@recipe('sling_scholar')
def _():
    tube('fork', [(-0.4, 0, 0.6), (-0.15, 0, 0.0), (0, 0, -0.2), (0.15, 0, 0.0), (0.4, 0, 0.6)], 0.07, P['wood'])
    tube('handle', [(0, 0, -0.2), (0, 0, -0.75)], 0.08, P['wood'])
    tube('band', [(-0.4, 0, 0.6), (0, -0.25, 0.35), (0.4, 0, 0.6)], 0.03, P['red'])
    box('cap', 0, 0, 0.75, 0.7, 0.7, 0.05, P['black'])
    cylinder('cap_b', 0, 0, 0.22, 0.6, 0.76, P['black'])
    tube('tassel', [(0.3, -0.3, 0.8), (0.4, -0.35, 0.6)], 0.025, P['gold_l'])

@recipe('lane_changer')
def _():
    for k, x in enumerate([-0.6, 0.0, 0.6]):
        box('guide%d' % k, x, 0, -0.5, 0.1, 1.4, 0.35, P['steel'])
    chrome_ball('ball', -0.3, -0.2, -0.25, 0.25)
    tube('swerve', [(-0.3, 0.3, -0.3), (0.0, 0.5, -0.1), (0.3, 0.3, -0.3)], 0.06, P['pink_l'])
    arrow_up('arrow', P['pink_l'], 0.5).location = (0.3, 0.1, 0.4)

@recipe('spin_doctor')
def _():
    tube('axle', [(-0.8, 0, 0.5), (0.8, 0, 0.5)], 0.05, P['steel'])
    p = box('plate', 0, 0, -0.5, 1.2, 0.08, 1.0, P['chrome'])
    rot(at(p, 0, 0, 0.5), 25, 0, 0)
    box('cross_v', 0, -0.1, -0.15, 0.18, 0.05, 0.6, P['red_l']).parent = p
    box('cross_h', 0, -0.1, 0.06, 0.6, 0.05, 0.18, P['red_l']).parent = p

@recipe('piggy_bank')
def _():
    scl(sphere('body', 0, 0, 0, 0.6, P['pink_l']), 1.25, 0.95, 0.9)
    rot(at(cylinder('snout', 0, 0, 0.2, 0, 0.18, P['pink']), 0.7, 0, 0.05), 0, 90, 0)
    for k, y in enumerate([-0.25, 0.25]):
        at(cylinder('ear%d' % k, 0, 0, 0.14, 0, 0.25, P['pink'], r_top=0.02), 0.4, y, 0.45)
        for j, x in enumerate([-0.4, 0.4]):
            cylinder('leg%d%d' % (k, j), x, y, 0.12, -0.75, -0.4, P['pink'])
    box('slot', 0, 0, 0.5, 0.35, 0.06, 0.06, P['black'])
    coin('c', 0, 0, 0.85, 0.25, tilt=90)

@recipe('ramp_rat')
def _():
    pts = [(-0.9, 0, -0.6), (-0.3, 0, -0.4), (0.2, 0, 0.0), (0.6, 0, 0.5)]
    for k, dy in enumerate([-0.3, 0.3]):
        tube('rail%d' % k, [(x, y + dy, z) for x, y, z in pts], 0.05, P['chrome'])
    scl(sphere('rat', 0.1, 0, -0.05, 0.25, P['dark']), 1.4, 1.0, 0.8)
    for k, y in enumerate([-0.12, 0.12]):
        sphere('ear%d' % k, 0.35, y, 0.15, 0.1, P['pink'])
    tube('tail', [(-0.2, 0, -0.1), (-0.5, 0.2, 0.0), (-0.7, 0.0, 0.2)], 0.025, P['pink'])

@recipe('coin_slot')
def _():
    box('panel', 0, 0.2, -0.8, 1.2, 0.2, 1.6, P['steel'])
    box('slot', 0, 0.08, 0.1, 0.12, 0.05, 0.5, P['black'])
    coin('c', 0, -0.05, 0.55, 0.32, tilt=90)
    box('light', 0, 0.08, -0.4, 0.5, 0.05, 0.2, P['fire_r'])

@recipe('insurance')
def _():
    pts = [(0, 0.9), (0.7, 0.6), (0.65, -0.1), (0, -0.9), (-0.65, -0.1), (-0.7, 0.6)]
    rot(prism('shield', pts, -0.12, 0.12, P['cyan']), 90, 0, 0)
    rot(prism('rimx', [(x * 0.8, y * 0.8) for x, y in pts], 0.12, 0.16, P['cyan_l']), 90, 0, 0)
    box('pv', 0, -0.2, -0.4, 0.2, 0.08, 0.8, P['white'])
    box('ph', 0, -0.2, -0.1, 0.8, 0.08, 0.2, P['white'])

@recipe('orbit_oracle')
def _():
    sphere('orb', 0, 0, 0.25, 0.55, P['glass'])
    sphere('core', 0, 0, 0.25, 0.2, P['glow'])
    rot(torus('ring', 0, 0, 0, 0.85, 0.04, P['gold_l']), 70, 0, 0).location = (0, 0, 0.25)
    lathe('stand', [(0.5, -0.8), (0.45, -0.6), (0.2, -0.45), (0.3, -0.3), (0.0, -0.3)], P['gold'])

@recipe('power_flippers')
def _():
    rot(at(flipper_prop('f'), -0.6, 0, -0.3), 0, 0, 20)
    at(bolt('b', P['fire'], 0.8), 0.4, -0.2, 0.3)

@recipe('drop_zone')
def _():
    for k, (x, dz) in enumerate([(-0.55, 0.0), (0.0, -0.55), (0.55, 0.0)]):
        box('t%d' % k, x, 0, -0.6 + dz, 0.45, 0.12, 0.9, P['gold_l'])
        box('s%d' % k, x, -0.07, -0.3 + dz, 0.3, 0.02, 0.1, P['white'])
    plate('floor', 0, 0, -0.65, 1.8, 0.8, 0.06, P['violet'])

@recipe('tilt_whisperer')
def _():
    c = box('cab', 0, 0, -0.4, 0.8, 1.4, 0.6, P['violet'])
    rot(c, 0, -12, 0)
    tube('wave1', [(0.6, -0.5, 0.5), (0.8, -0.4, 0.7), (1.0, -0.5, 0.9)], 0.04, P['cyan_l'])
    scl(rot(at(sphere('feather', 0, 0, 0, 0.35, P['white']), -0.5, -0.4, 0.6), 0, 40, 0), 1.0, 0.25, 0.35)

@recipe('bonus_round')
def _():
    box('gift', 0, 0, -0.7, 1.1, 1.1, 1.0, P['pink'])
    box('rib1', 0, 0, -0.71, 0.2, 1.12, 1.02, P['gold_l'])
    box('rib2', 0, 0, -0.71, 1.12, 0.2, 1.02, P['gold_l'])
    for k, a in enumerate([30, -30]):
        rot(at(torus('bow%d' % k, 0, 0, 0, 0.22, 0.06, P['gold_l']), 0.15 * (1 if k else -1), 0, 0.42), 90, a, 0)

@recipe('spare_change')
def _():
    coin('c1', -0.3, 0.1, -0.5, 0.35)
    coin('c2', 0.25, -0.1, -0.45, 0.32, m=P['steel'])
    coin('c3', 0.0, 0.2, -0.05, 0.3, tilt=60)

@recipe('standup_comic')
def _():
    sphere('mic', 0, 0, 0.55, 0.28, P['steel'])
    cylinder('mic_h', 0, 0, 0.1, -0.3, 0.3, P['black'])
    cylinder('stand', 0, 0, 0.04, -0.9, -0.3, P['chrome'])
    box('target', 0.6, 0, -0.6, 0.35, 0.1, 0.6, P['pink'])

@recipe('toll_booth')
def _():
    box('booth', -0.5, 0, -0.8, 0.6, 0.6, 1.0, P['cream'])
    prism('roof', [(-0.85, -0.4), (-0.15, -0.4), (-0.15, 0.4), (-0.85, 0.4)], 0.2, 0.3, P['red'])
    box('window', -0.5, -0.31, -0.3, 0.35, 0.02, 0.3, P['glass'])
    for k in range(4):
        box('arm%d' % k, 0.05 + k * 0.25, 0, -0.25, 0.25, 0.08, 0.08, P['red_l'] if k % 2 else P['white'])

@recipe('late_bloomer')
def _():
    tube('stem', [(0, 0, -0.9), (0.05, 0, -0.3), (0, 0, 0.2)], 0.05, P['green'])
    for k in range(6):
        a = 2 * math.pi * k / 6
        scl(at(sphere('pet%d' % k, 0, 0, 0, 0.2, P['pink_l']), math.cos(a) * 0.25, -0.05, 0.3 + math.sin(a) * 0.25), 1.0, 0.4, 1.0)
    sphere('mid', 0, -0.1, 0.3, 0.14, P['gold_l'])
    scl(rot(at(sphere('leaf', 0, 0, 0, 0.2, P['green_l']), 0.2, 0, -0.4), 0, 40, 0), 1.0, 0.3, 0.6)

@recipe('chalk_line')
def _():
    plate('board', 0, 0.1, -0.9, 1.7, 0.1, 1.6, P['green'])
    box('line', 0, 0, -0.1, 1.3, 0.02, 0.06, P['white'])
    rot(at(box('chalk', 0, 0, -0.08, 0.45, 0.14, 0.14, P['white']), 0.4, -0.2, 0.25), 0, -20, 30)

@recipe('even_keel')
def _():
    cylinder('post', 0, 0, 0.05, -0.8, 0.5, P['gold'])
    box('beam', 0, 0, 0.45, 1.6, 0.08, 0.08, P['gold'])
    for k, x in enumerate([-0.7, 0.7]):
        tube('ch%d' % k, [(x, 0, 0.45), (x, 0, 0.0)], 0.015, P['steel'])
        lathe('pan%d' % k, [(0.0, -0.05), (0.3, 0.0), (0.32, 0.05)], P['gold_l'], x=x, z0=-0.05)
    lathe('base', [(0.4, -0.85), (0.3, -0.75), (0.0, -0.75)], P['gold'])

@recipe('first_strike')
def _():
    at(bolt('b', P['fire'], 1.1), -0.25, 0, 0)
    glyph('one', '1', 1.0, P['white'], font=FONT_BOLD, depth=0.15, x=0.45, y=0, z=-0.4)

@recipe('rubber_band')
def _():
    scl(torus('band', 0, 0, 0, 0.6, 0.09, P['red']), 1.4, 0.7, 1.0)
    for k, x in enumerate([-0.85, 0.85]):
        cylinder('post%d' % k, x, 0, 0.1, -0.5, 0.4, P['chrome'])

@recipe('pinwheel')
def _():
    tube('stick', [(0, 0.1, -1.0), (0, 0.1, 0.1)], 0.04, P['wood_l'])
    cols = [P['pink'], P['cyan'], P['gold_l'], P['green_l']]
    for k in range(4):
        a = math.pi / 2 * k
        tri = [(0, 0), (math.cos(a) * 0.7, math.sin(a) * 0.7), (math.cos(a + 0.9) * 0.45, math.sin(a + 0.9) * 0.45)]
        rot(prism('blade%d' % k, tri, -0.02, 0.02, cols[k]), 90, 0, 0).location = (0, 0, 0.15)
    sphere('pin', 0, -0.05, 0.15, 0.07, P['gold'])

@recipe('bell_ringer')
def _():
    lathe('bell', [(0.0, 0.7), (0.15, 0.68), (0.32, 0.5), (0.4, 0.1), (0.55, -0.35), (0.62, -0.45), (0.0, -0.45)], P['gold'])
    sphere('clap', 0, 0, -0.55, 0.13, P['gold_l'])
    torus('loop', 0, 0, 0.75, 0.12, 0.04, P['gold'])

@recipe('sharpshooter')
def _():
    for k, (r, m) in enumerate([(0.8, P['white']), (0.62, P['red']), (0.44, P['white']), (0.26, P['red'])]):
        rot(at(cylinder('ring%d' % k, 0, 0, r, 0, 0.05 + k * 0.02, m), 0, 0.1, 0), 90, 0, 0)
    rot(at(tube('arrow', [(0, 0, 0), (0, 0, 1.0)], 0.04, P['wood_l']), 0.05, -0.1, 0.05), -100, 30, 0)

@recipe('bargain_bin')
def _():
    box('bin', 0, 0, -0.8, 1.3, 0.9, 0.7, P['steel'])
    box('in', 0, 0, -0.15, 1.15, 0.75, 0.02, P['dark'])
    box('tag', 0.45, -0.46, -0.45, 0.45, 0.02, 0.3, P['gold_l'])
    glyph('pct', '%', 0.3, P['red'], font=FONT_BOLD, depth=0.02, x=0.45, y=-0.48, z=-0.42)
    coin('c', -0.2, 0, 0.0, 0.25, tilt=30)

@recipe('night_shift')
def _():
    sphere('moon', 0, 0, 0.1, 0.65, P['gold_l'])
    sphere('shade', 0.35, -0.25, 0.25, 0.55, P['violet'])
    for k, (x, z) in enumerate([(-0.7, 0.7), (0.75, -0.5), (-0.5, -0.65)]):
        glyph('star%d' % k, '★', 0.35, P['glow'], x=x, y=-0.3, z=z)

@recipe('warm_up')
def _():
    lathe('mug', [(0.0, -0.6), (0.45, -0.6), (0.48, 0.25), (0.42, 0.25), (0.4, -0.5), (0.0, -0.5)], P['red'])
    rot(at(torus('handle', 0, 0, 0, 0.22, 0.06, P['red']), 0.52, 0, -0.15), 90, 0, 0)
    for k, x in enumerate([-0.15, 0.15]):
        tube('steam%d' % k, [(x, 0, 0.35), (x + 0.1, 0, 0.55), (x - 0.05, 0, 0.75), (x + 0.08, 0, 0.95)], 0.035, P['white'])

@recipe('long_ball')
def _():
    scl(chrome_ball('b', 0, 0, 0, 0.45), 1.6, 1.0, 1.0)
    for k, z in enumerate([-0.25, 0.0, 0.25]):
        box('line%d' % k, -1.0, 0, z - 0.03, 0.5, 0.05, 0.06, P['cyan_l'])

@recipe('combo_breaker')
def _():
    for k in range(3):
        rot(at(scl(torus('link%d' % k, 0, 0, 0, 0.28, 0.08, P['steel']), 1.4, 1.0, 1.0), -0.7 + k * 0.6, 0, 0), 90 if k % 2 else 0, 0, 0)
    at(bolt('crack', P['fire'], 0.5), 0.75, -0.2, 0.3)

@recipe('momentum')
def _():
    r = cylinder('body', 0, 0, 0.3, -0.5, 0.4, P['white'])
    cylinder('nose', 0, 0, 0.3, 0.4, 0.85, P['red'], r_top=0.02).parent = r
    for k in range(3):
        a = 2 * math.pi * k / 3
        f = prism('fin%d' % k, [(0.28, -0.05), (0.6, 0.0), (0.28, 0.05)], -0.5, -0.15, P['red'])
        f.rotation_euler = (0, 0, a)
        f.parent = r
    cylinder('flame', 0, 0, 0.2, -0.95, -0.5, P['fire'], r_top=0.25).parent = r
    rot(r, 0, -35, 0)

@recipe('multiball_maniac')
def _():
    for k, (x, z) in enumerate([(-0.45, -0.3), (0.45, -0.3), (0.0, 0.4)]):
        chrome_ball('b%d' % k, x, 0, z, 0.38)
    glyph('ex', '!', 0.6, P['pink_l'], font=FONT_BOLD, depth=0.1, x=0.75, y=-0.3, z=0.35)

@recipe('glass_cannon')
def _():
    b = cylinder('barrel', 0, 0, 0.3, -0.7, 0.8, P['glass'], r_top=0.24)
    rot(at(b, 0, 0, -0.1), 0, 60, 0)
    torus('band', 0, 0, 0, 0.3, 0.05, P['gold']).parent = b
    for k, y in enumerate([-0.35, 0.35]):
        rot(at(torus('wheel%d' % k, 0, 0, 0, 0.32, 0.06, P['wood']), -0.15, y, -0.45), 90, 0, 0)
    chrome_ball('shot', 0.85, 0, 0.45, 0.15)

@recipe('hot_streak')
def _():
    for k, (r, h, m) in enumerate([(0.55, 1.1, P['fire_r']), (0.38, 0.95, P['fire']), (0.2, 0.65, P['glow'])]):
        cylinder('fl%d' % k, 0, -k * 0.05, r, -0.7, -0.7 + h, m, r_top=0.0)
        sphere('fb%d' % k, 0, -k * 0.05, -0.55, r, m)

@recipe('compound_interest')
def _():
    for k in range(5):
        coin('c%d' % k, -0.4, 0, -0.8 + k * 0.12, 0.35)
    for k in range(3):
        coin('d%d' % k, 0.35, 0, -0.8 + k * 0.12, 0.3)
    at(arrow_up('up', P['green_l'], 0.8), 0.0, -0.2, 0.35)

@recipe('lucky_drain')
def _():
    verts = [(0.55 * math.cos(a), 0, 0.55 * math.sin(a)) for a in [math.radians(-30 + i * 12) for i in range(21)]]
    tube('shoe', verts, 0.12, P['gold'], seg=10)
    for k, a in enumerate([-10, 60, 120, 190]):
        sphere('nail%d' % k, 0.55 * math.cos(math.radians(a)), -0.12, 0.55 * math.sin(math.radians(a)), 0.05, P['steel'])

@recipe('fat_stack')
def _():
    for k in range(6):
        b = box('bill%d' % k, 0, 0, -0.7 + k * 0.13, 1.3, 0.7, 0.12, P['green_l'] if k % 2 else P['green'])
        b.rotation_euler = (0, 0, math.radians((k * 7) % 13 - 6))
    box('band', 0, 0, -0.72, 0.25, 0.72, 0.8, P['gold_l'])

@recipe('wild_lanes')
def _():
    c = card_prop('card', '★', P['pink'], tilt=72, rz=-10)
    glyph('w', 'W', 0.45, P['violet'], font=FONT_BOLD, depth=0.03, x=-0.25, y=-0.55, z=0.3)

@recipe('abacus')
def _():
    for k, x in enumerate([-0.8, 0.8]):
        box('side%d' % k, x, 0, -0.8, 0.12, 0.3, 1.6, P['wood'])
    for r in range(3):
        z = -0.4 + r * 0.4
        tube('rod%d' % r, [(-0.8, 0, z), (0.8, 0, z)], 0.025, P['steel'])
        for b in range(4):
            scl(sphere('bead%d%d' % (r, b), -0.5 + b * 0.18 + (0.4 if b > 1 and r % 2 else 0), 0, z, 0.1, [P['red'], P['cyan'], P['gold_l']][r]), 0.8, 1.0, 1.0)

@recipe('bank_robber')
def _():
    scl(sphere('bag', 0, 0, -0.25, 0.6, P['cream']), 1.0, 1.0, 1.1)
    cylinder('neck', 0, 0, 0.18, 0.35, 0.55, P['cream'], r_top=0.28)
    torus('tie', 0, 0, 0.4, 0.2, 0.05, P['wood'])
    glyph('d', '$', 0.6, P['green'], font=FONT_BOLD, depth=0.06, x=0.0, y=-0.6, z=-0.3)
    box('mask', 0, -0.62, 0.05, 0.8, 0.04, 0.12, P['black'])

@recipe('overdrive')
def _():
    rot(cylinder('dial', 0, 0, 0.8, 0, 0.12, P['dark']), 90, 0, 0)
    rot(torus('rim', 0, 0, 0, 0.8, 0.06, P['chrome']), 90, 0, 0)
    for k in range(7):
        a = math.radians(210 - k * 40)
        box('tick%d' % k, math.cos(a) * 0.62, -0.14, math.sin(a) * 0.62 - 0.03, 0.06, 0.02, 0.12, P['red_l'] if k > 4 else P['white'])
    rot(at(box('needle', 0.25, -0.16, -0.025, 0.55, 0.02, 0.05, P['red_l']), 0, 0, 0), 0, -40, 0)

@recipe('swashbuckler')
def _():
    s = box('blade', 0, 0, -0.1, 0.12, 0.04, 1.4, P['chrome'])
    box('guard', 0, 0, -0.15, 0.5, 0.08, 0.08, P['gold']).parent = s
    box('grip', 0, 0, -0.55, 0.1, 0.1, 0.4, P['wood']).parent = s
    sphere('pommel', 0, 0, -0.6, 0.08, P['gold']).parent = s
    rot(s, 0, 35, 0)
    rot(at(cylinder('hat', 0, 0, 0.5, 0, 0.12, P['black']), -0.45, 0, 0.45), 0, -20, 0)

@recipe('metronome')
def _():
    prism('body', [(-0.55, -0.15), (0.55, -0.15), (0.2, 0.15), (-0.2, 0.15)], -0.8, -0.75, P['wood'])
    p = prism('pyr', [(-0.5, -0.3), (0.5, -0.3), (0.5, 0.3), (-0.5, 0.3)], -0.8, -0.2, P['wood'])
    cylinder('top', 0, 0, 0.18, -0.2, 0.75, P['wood_l'], r_top=0.05)
    rot(at(tube('arm', [(0, 0, 0), (0, 0, 1.2)], 0.03, P['chrome']), 0, -0.32, -0.6), 0, 25, 0)
    sphere('weight', 0.3, -0.33, 0.1, 0.09, P['gold'])

@recipe('featherweight')
def _():
    pts = [(math.sin(t) * 0.25 * (1 - t / 3), 1.2 * (t / 3) - 0.6) for t in [i * 0.15 for i in range(21)]]
    outline = pts + [(-x, y) for x, y in reversed(pts)]
    rot(prism('vane', outline, -0.02, 0.02, P['white']), 90, 0, 25)
    rot(tube('shaft', [(0, -0.05, -0.75), (0, -0.05, 0.65)], 0.025, P['cream']), 0, 25, 0)

@recipe('echo_chamber')
def _():
    lathe('cone', [(0.1, -0.2), (0.65, 0.35), (0.7, 0.4), (0.0, 0.4)], P['dark'])
    rot(at(cylinder('mag', 0, 0, 0.2, -0.6, -0.2, P['steel']), 0, 0, 0), 0, 0, 0)
    for k, r in enumerate([0.85, 1.05]):
        verts = [(r * math.cos(a), 0, 0.2 + r * math.sin(a) * 0.6) for a in [math.radians(-50 + i * 10) for i in range(11)]]
        tube('wave%d' % k, verts, 0.035, P['cyan_l'])
    for o in bpy.data.objects:
        if o.name in ('cone', 'mag'):
            o.rotation_euler = (0, math.radians(90), 0)

@recipe('marathon')
def _():
    lathe('cup', [(0.0, -0.1), (0.5, 0.0), (0.55, 0.6), (0.48, 0.6), (0.44, 0.05), (0.0, 0.0)], P['gold'])
    lathe('stem', [(0.08, -0.45), (0.08, -0.1)], P['gold'])
    box('base', 0, 0, -0.75, 0.6, 0.45, 0.3, P['wood'])
    for k, x in enumerate([-0.55, 0.55]):
        rot(at(torus('h%d' % k, 0, 0, 0, 0.18, 0.04, P['gold']), x, 0, 0.3), 90, 0, 0)

@recipe('collector')
def _():
    box('shelf', 0, 0, -0.3, 1.8, 0.5, 0.08, P['wood'])
    box('shelf2', 0, 0, -0.95, 1.8, 0.5, 0.08, P['wood'])
    chrome_ball('a', -0.55, 0, -0.07, 0.18)
    coin('b', 0.0, 0, 0.0, 0.2, tilt=90)
    pip_cube('c', 0.55, 0, -0.1, 0.32, 5, P['white'], P['black'])
    lathe('d', [(0.0, 0.0), (0.15, 0.05), (0.08, 0.3), (0.0, 0.32)], P['glass'], x=-0.3, z0=-0.87)
    sphere('e', 0.35, 0, -0.75, 0.15, P['pink_l'])

@recipe('dividend')
def _():
    for k, (a0, a1, m) in enumerate([(0, 140, P['green_l']), (140, 250, P['cyan']), (250, 360, P['gold_l'])]):
        pts = [(0, 0)] + [(0.75 * math.cos(math.radians(a)), 0.75 * math.sin(math.radians(a))) for a in range(a0, a1 + 1, 10)]
        o = prism('slice%d' % k, pts, -0.15, 0.15 + (0.15 if k == 0 else 0), m)
        if k == 0:
            mid = math.radians((a0 + a1) / 2)
            o.location = (math.cos(mid) * 0.15, math.sin(mid) * 0.15, 0)
        rot(o, 60, 0, 0)

@recipe('pressure_plate')
def _():
    cylinder('base', 0, 0, 0.8, -0.6, -0.45, P['steel'])
    cylinder('btn', 0, 0, 0.55, -0.45, -0.3, P['red_l'])
    for k in range(4):
        a = math.pi / 2 * k + math.pi / 4
        ar = arrow_up('ar%d' % k, P['gold_l'], 0.35)
        ar.rotation_euler = (math.radians(180), 0, 0)
        ar.location = (math.cos(a) * 0.55, math.sin(a) * 0.55, 0.3)

@recipe('ricochet')
def _():
    for k, x in enumerate([-0.85, 0.85]):
        box('wall%d' % k, x, 0, -0.8, 0.12, 0.6, 1.6, P['steel'])
    tube('path', [(-0.75, 0, -0.7), (0.75, 0, -0.3), (-0.75, 0, 0.1), (0.75, 0, 0.5)], 0.03, P['pink_l'])
    chrome_ball('b', 0.6, 0, 0.55, 0.18)

@recipe('chrome_dome')
def _():
    dome('dome', 0, 0, 0.8, -0.4, 0.8, P['chrome'])
    cylinder('rim', 0, 0, 0.85, -0.5, -0.4, P['dark'])
    sphere('glint', -0.3, -0.4, 0.3, 0.06, P['glow'])

@recipe('double_down')
def _():
    rot(pip_cube('d1', -0.4, 0, -0.2, 0.7, 6, P['white'], P['red']), 10, 0, 20)
    rot(pip_cube('d2', 0.45, -0.1, -0.3, 0.7, 6, P['red'], P['white']), -5, 0, -15)

@recipe('blueprint')
def _():
    p = plate('sheet', 0, 0, -0.05, 1.7, 1.3, 0.04, P['blueprint'])
    rot(p, 55, 0, 0)
    for k in range(5):
        box('gl%d' % k, -0.6 + k * 0.3, 0, -0.01, 0.015, 1.1, 0.03, P['white']).parent = p
    tube('drawn', [(-0.4, -0.3, 0.03), (0.3, -0.3, 0.03), (0.3, 0.3, 0.03), (-0.4, 0.3, 0.03), (-0.4, -0.3, 0.03)], 0.02, P['white']).parent = p

@recipe('brainstorm')
def _():
    for k in range(9):
        a = 2 * math.pi * k / 9
        sphere('lobe%d' % k, math.cos(a) * 0.35, math.sin(a) * 0.2, 0.1 + math.sin(a * 2) * 0.12, 0.32, P['pink_l'])
    sphere('core', 0, 0, 0.1, 0.4, P['pink'])
    at(bolt('b', P['fire'], 0.45), 0.55, -0.4, 0.65)

@recipe('pinpoint')
def _():
    for k, (r, m) in enumerate([(0.8, P['cyan']), (0.55, P['white']), (0.3, P['cyan'])]):
        cylinder('ring%d' % k, 0, 0, r, -0.6 + k * 0.03, -0.55 + k * 0.03, m)
    tube('needle', [(0, 0, -0.5), (0.15, 0, 0.4)], 0.03, P['chrome'])
    sphere('head', 0.17, 0, 0.5, 0.17, P['red_l'])

@recipe('black_hole')
def _():
    sphere('hole', 0, 0, 0, 0.45, P['black'])
    scl(torus('disk', 0, 0, 0, 0.75, 0.12, P['fire']), 1.0, 1.0, 0.25)
    scl(torus('disk2', 0, 0, 0, 0.95, 0.06, P['fire_r']), 1.0, 1.0, 0.2)
    for o in bpy.data.objects:
        if o.name.startswith('disk'):
            o.rotation_euler = (math.radians(-15), 0, 0)

@recipe('the_house')
def _():
    box('walls', 0, 0, -0.8, 1.1, 0.9, 0.8, P['cream'])
    roof = [(-0.65, 0), (0.65, 0), (0, 0.55)]
    o = prism('roof', roof, -0.5, 0.5, P['red'])
    o.rotation_euler = (math.radians(90), 0, 0)
    o.location = (0, 0, 0.0)
    box('door', 0, -0.46, -0.8, 0.28, 0.02, 0.45, P['wood'])
    glyph('d', '$', 0.4, P['gold_l'], font=FONT_BOLD, depth=0.04, x=0.0, y=-0.47, z=0.25)

@recipe('perpetual_motion')
def _():
    rot(torus('wheel', 0, 0, 0, 0.75, 0.06, P['wood']), 90, 0, 0)
    for k in range(8):
        a = 2 * math.pi * k / 8
        tube('sp%d' % k, [(0, 0, 0), (math.cos(a) * 0.75, 0, math.sin(a) * 0.75)], 0.025, P['wood_l'])
        chrome_ball('b%d' % k, math.cos(a) * (0.75 + (0.15 if math.cos(a) > 0 else -0.05)), -0.05, math.sin(a) * 0.75, 0.11)
    cylinder('hub', 0, 0, 0.1, -0.1, 0.1, P['steel']).rotation_euler = (math.radians(90), 0, 0)

@recipe('golden_ticket')
def _():
    pts = [(-0.85, -0.45), (0.85, -0.45), (0.85, -0.12), (0.72, 0.0), (0.85, 0.12), (0.85, 0.45), (-0.85, 0.45), (-0.85, 0.12), (-0.72, 0.0), (-0.85, -0.12)]
    t = prism('ticket', pts, -0.03, 0.03, P['gold_l'])
    rot(t, 70, 0, -12)
    glyph('star', '★', 0.45, P['red'], depth=0.02, x=0.0, y=-0.12, z=0.0)

@recipe('infinity_ramp')
def _():
    pts = []
    for i in range(65):
        a = 2 * math.pi * i / 64
        pts.append((0.8 * math.cos(a) / (1 + math.sin(a) ** 2), 0.8 * math.sin(a) * math.cos(a) / (1 + math.sin(a) ** 2), 0.15 * math.sin(a)))
    tube('loop', pts, 0.1, P['cyan_l'], seg=10)
    tube('loop_rail', [(x, y, z + 0.11) for x, y, z in pts], 0.03, P['chrome'])
    chrome_ball('b', 0.55, 0.12, 0.2, 0.13)

# ---- cards: chips are poker chips with a shot emblem
def chip(colour, emblem):
    def build():
        c = cylinder('chip', 0, 0, 0.85, -0.12, 0.12, P[colour], seg=36)
        for k in range(8):
            a = 2 * math.pi * k / 8
            box('notch%d' % k, math.cos(a) * 0.78, math.sin(a) * 0.78, -0.125, 0.18, 0.12, 0.25, P['white']).rotation_euler = (0, 0, a)
        cylinder('face', 0, 0, 0.58, 0.12, 0.14, P['white'], seg=36)
        emblem()
        for o in bpy.data.objects:
            if o.type == 'MESH':
                o.rotation_euler = (o.rotation_euler[0] + math.radians(65), o.rotation_euler[1], o.rotation_euler[2])
    return build
def em_bumper():
    cylinder('e1', 0, 0, 0.32, 0.14, 0.3, P['cyan'])
    cylinder('e2', 0, 0, 0.36, 0.3, 0.36, P['cyan_l'])
def em_sling():
    prism('e1', [(-0.3, -0.25), (0.3, -0.25), (-0.15, 0.35)], 0.14, 0.24, P['pink'])
def em_spin():
    box('e1', 0, 0, 0.14, 0.6, 0.12, 0.08, P['steel'])
    box('e2', 0, 0, 0.22, 0.12, 0.45, 0.08, P['steel'])
def em_lanes():
    for k, x in enumerate([-0.25, 0.0, 0.25]):
        box('e%d' % k, x, 0, 0.14, 0.1, 0.6, 0.12, P['gold_l'])
def em_bank():
    for k, x in enumerate([-0.25, 0.0, 0.25]):
        box('e%d' % k, x, 0, 0.14, 0.2, 0.1, 0.35, P['gold_l'])
def em_standup():
    box('e1', 0, 0, 0.14, 0.4, 0.1, 0.35, P['pink'])
def em_ramp():
    tube('e1', [(-0.35, -0.2, 0.15), (0.0, 0.0, 0.25), (0.3, 0.3, 0.4)], 0.07, P['cyan_l'])
def em_orbit():
    torus('e1', 0, 0, 0.2, 0.32, 0.06, P['cyan_l'])
    sphere('e2', 0.32, 0, 0.2, 0.09, P['chrome'])
def em_combo():
    for k, x in enumerate([-0.18, 0.18]):
        o = arrow_up('e%d' % k, P['violet_l'], 0.4)
        o.location = (x, 0, 0.3)
for cid, col, em in [('chip_bumper', 'cyan', em_bumper), ('chip_lower', 'pink', em_sling), ('chip_spin', 'steel', em_spin),
                     ('chip_lanes', 'gold', em_lanes), ('chip_bank', 'orange', em_bank), ('chip_standup', 'pink', em_standup),
                     ('chip_ramp', 'cyan', em_ramp), ('chip_orbit', 'violet', em_orbit), ('chip_combo', 'violet', em_combo)]:
    R[cid] = chip(col, em)

def paint_can(colour):
    def build():
        lathe('can', [(0.0, -0.75), (0.55, -0.75), (0.55, 0.3), (0.0, 0.3)], P['steel'])
        cylinder('label', 0, 0, 0.56, -0.5, 0.05, P[colour])
        cylinder('paint', 0, 0, 0.5, 0.3, 0.33, P[colour])
        tube('drip', [(0.5, -0.3, 0.32), (0.56, -0.3, 0.0), (0.57, -0.3, -0.2)], 0.05, P[colour])
        tube('bail', [(-0.55, 0, 0.1), (0.0, 0, 0.7), (0.55, 0, 0.1)], 0.025, P['steel'])
    return build
R['glass_coat'] = paint_can('glass')
R['blue_paint'] = paint_can('cyan')
R['red_paint'] = paint_can('red')

@recipe('gold_leaf')
def _():
    pts = [(0, -0.8), (0.4, -0.3), (0.5, 0.2), (0.0, 0.8), (-0.5, 0.2), (-0.4, -0.3)]
    rot(prism('leaf', pts, -0.03, 0.03, P['gold_l']), 60, 0, -20)
    tube('vein', [(0, -0.75, 0.05), (0, 0.7, 0.05)], 0.02, P['gold']).rotation_euler = (math.radians(60), 0, math.radians(-20))

@recipe('lucky_charm')
def _():
    for k in range(4):
        a = math.pi / 2 * k + math.pi / 4
        scl(at(sphere('leaf%d' % k, 0, 0, 0, 0.3, P['green_l']), math.cos(a) * 0.3, -0.05, math.sin(a) * 0.3 + 0.15), 1.0, 0.35, 1.0)
    tube('stem', [(0, 0, 0.1), (0.1, 0, -0.5), (0.25, 0, -0.85)], 0.04, P['green'])

@recipe('multiball')
def _():
    for k, (x, z) in enumerate([(-0.5, -0.35), (0.5, -0.35), (0.0, 0.45)]):
        chrome_ball('b%d' % k, x, 0, z, 0.36)
    for k in range(6):
        a = 2 * math.pi * k / 6
        box('ray%d' % k, math.cos(a) * 0.95, 0.3, math.sin(a) * 0.95, 0.25, 0.03, 0.06, P['glow']).rotation_euler = (0, -a, 0)

@recipe('safety_net')
def _():
    for k in range(6):
        x = -0.75 + k * 0.3
        tube('v%d' % k, [(x, 0, -0.6), (x, 0.2, 0.0), (x, 0, 0.6)], 0.025, P['white'])
    for k in range(5):
        z = -0.6 + k * 0.3
        tube('h%d' % k, [(-0.8, 0, z), (0.0, 0.25 - abs(z) * 0.2, z), (0.8, 0, z)], 0.025, P['white'])
    chrome_ball('b', 0.1, -0.3, 0.2, 0.25)

@recipe('photocopy')
def _():
    box('copier', 0, 0, -0.8, 1.5, 1.0, 0.6, P['steel'])
    box('lid', 0, 0.05, -0.2, 1.45, 0.9, 0.08, P['dark'])
    box('glow', 0, -0.51, -0.5, 1.1, 0.02, 0.1, P['glow'])
    for k in range(2):
        rot(at(box('paper%d' % k, 0, 0, 0, 0.8, 1.0, 0.02, P['paper']), 0.15 * k, -0.3, 0.1 + k * 0.25), 30, 0, 10 - k * 15)

@recipe('hermit')
def _():
    lathe('lantern', [(0.0, -0.7), (0.4, -0.7), (0.35, -0.6), (0.3, 0.2), (0.4, 0.3), (0.15, 0.5), (0.0, 0.5)], P['dark'])
    sphere('flame', 0, -0.35, -0.2, 0.22, P['glow'])
    for k in range(4):
        a = math.pi / 2 * k
        tube('bar%d' % k, [(math.cos(a) * 0.32, math.sin(a) * 0.32, -0.6), (math.cos(a) * 0.3, math.sin(a) * 0.3, 0.2)], 0.03, P['gold'])
    torus('ring', 0, 0, 0.6, 0.15, 0.04, P['gold'])

@recipe('prism_wheel')
def _():
    o = prism('prism', [(-0.5, -0.3), (0.5, -0.3), (0.0, 0.55)], -0.5, 0.5, P['glass'])
    rot(o, 90, 0, 0)
    for k, m in enumerate(['red_l', 'gold_l', 'green_l', 'cyan_l', 'violet_l']):
        box('beam%d' % k, 0.85, 0, -0.35 + k * 0.12, 0.7, 0.04, 0.1, P[m])
    box('in', -0.9, 0, 0.05, 0.6, 0.03, 0.06, P['white'])

@recipe('recycler')
def _():
    for k in range(3):
        a0 = 2 * math.pi * k / 3
        pts = [(0.65 * math.cos(a0 + t * 0.12), 0, 0.65 * math.sin(a0 + t * 0.12)) for t in range(14)]
        tube('arc%d' % k, pts, 0.09, P['green_l'])
        tip = pts[-1]
        ang = a0 + 13 * 0.12
        o = prism('head%d' % k, [(-0.2, 0), (0.2, 0), (0, 0.3)], -0.06, 0.06, P['green_l'])
        o.location = tip
        o.rotation_euler = (math.radians(90), -ang - math.pi / 2, 0)

@recipe('overclock')
def _():
    box('chip', 0, 0, -0.4, 1.1, 1.1, 0.2, P['dark'])
    box('die', 0, 0, -0.2, 0.55, 0.55, 0.06, P['steel'])
    for k in range(6):
        for s in (-1, 1):
            box('pin%d%d' % (k, s), -0.45 + k * 0.18, s * 0.62, -0.38, 0.06, 0.16, 0.04, P['gold'])
            box('pinx%d%d' % (k, s), s * 0.62, -0.45 + k * 0.18, -0.38, 0.16, 0.06, 0.04, P['gold'])
    at(bolt('b', P['fire'], 0.5), 0.0, -0.3, 0.45)

@recipe('fortune_cookie')
def _():
    for k, s in enumerate([-1, 1]):
        o = dome('half%d' % k, 0, 0, 0.55, 0.0, 0.5, P['cream'])
        o.rotation_euler = (math.radians(90), math.radians(30 * s), 0)
        o.location = (0.12 * s, 0, -0.1)
    box('slip', 0.3, -0.35, -0.2, 0.7, 0.02, 0.16, P['paper'])

@recipe('sledgehammer')
def _():
    h = box('head', 0, 0, 0.45, 0.9, 0.38, 0.38, P['steel'])
    tube('handle', [(0, 0, 0.5), (0, 0, -0.9)], 0.07, P['wood_l'])
    for o in bpy.data.objects:
        if o.name in ('head', 'handle'):
            o.rotation_euler = (0, math.radians(-35), 0)

@recipe('tune_up')
def _():
    w = box('shaft', 0, 0, -0.06, 1.3, 0.16, 0.12, P['chrome'])
    for k, x in enumerate([-0.75, 0.75]):
        o = torus('jaw%d' % k, x, 0, 0, 0.18, 0.08, P['chrome'])
    for o in bpy.data.objects:
        if o.name in ('shaft', 'jaw0', 'jaw1'):
            o.rotation_euler = (math.radians(60), 0, math.radians(-30))
    at(flipper_prop('f'), -0.4, 0.4, -0.7).scale = (0.5, 0.5, 0.5)

# ------------------------------------------------------------------ render loop
def clear_scene():
    for o in list(bpy.data.objects):
        if o.name not in RIG:
            bpy.data.objects.remove(o, do_unlink=True)
    for me in list(bpy.data.meshes):
        if me.users == 0:
            bpy.data.meshes.remove(me)

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

def material_ramp(o):
    if o.type != 'MESH' or not o.data.materials:
        return 0
    m = o.data.materials[0]
    p = next((n for n in m.node_tree.nodes if n.type == 'BSDF_PRINCIPLED'), None)
    if p is None:
        return 0
    col = list(p.inputs['Base Color'].default_value)[:3]
    if p.inputs['Emission Strength'].default_value > 1.0:
        col = list(p.inputs['Emission Color'].default_value)[:3]
    if p.inputs['Metallic'].default_value > 0.5 and max(col) - min(col) < 0.15:
        return 0
    s = [c * 12.92 if c <= 0.0031308 else 1.055 * c ** (1 / 2.4) - 0.055 for c in col]
    return pixel.nearest_ramp(s)

from mathutils import Vector
def frame(objs):
    """fit the subject to the frame: ortho scale and lens shift from its projected bounds"""
    inv = cam.matrix_world.inverted()
    xs, ys = [], []
    for o in objs:
        for c in o.bound_box:
            p = inv @ (o.matrix_world @ Vector(c))
            xs.append(p.x); ys.append(p.y)
    if not xs:
        return
    w, h = max(xs) - min(xs), max(ys) - min(ys)
    size = max(w, h) * 1.12 + 0.05
    cam_data.ortho_scale = size
    cam_data.shift_x = ((max(xs) + min(xs)) / 2) / size
    cam_data.shift_y = ((max(ys) + min(ys)) / 2) / size

done = 0
for cid, build in R.items():
    if ONLY and cid not in ONLY:
        continue
    clear_scene()
    build()
    bpy.context.view_layer.update()
    objs = [o for o in bpy.data.objects if o.type == 'MESH']
    frame(objs)
    for i, o in enumerate(objs):
        o.pass_index = (i + 2) * 16 + material_ramp(o)
    path = os.path.join(TMP, cid + '.png')
    sc.render.filepath = path
    bpy.ops.render.render(write_still=True)
    rgba = read_png(path)
    vl = bpy.context.view_layer
    vl.material_override = idm
    sc.render.filter_size = 0.01
    sc.render.filepath = os.path.join(TMP, cid + '_id.png')
    bpy.ops.render.render(write_still=True)
    vl.material_override = None
    sc.render.filter_size = 1.5
    ids, ramps = pixel.decode_ids(read_png(os.path.join(TMP, cid + '_id.png')))
    art = pixel.pixelize(rgba, K, ids=ids, ramps=ramps, alpha='hard', outer_outline=True, inner_outline=True)
    save_png(os.path.join(OUT, cid + '.png'), art)
    done += 1
    print('CARD', cid, flush=True)
print('DONE', done)
