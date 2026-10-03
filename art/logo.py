# The title logo: extruded, bevelled letters with gradient faces, a chrome pinball on a gold swoosh,
# and sparkles, rendered and pixelized like the rest of the art.
#   blender --background --python art/logo.py
# Output: assets/logo.png
import bpy, bmesh, sys, os, math
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import pixel
from geom import mat, srgb, sphere, tube, make, load_font

ROOT = os.path.dirname(HERE)
OUT = os.path.join(ROOT, 'assets', 'logo.png')
TMP = os.path.join(ROOT, 'build', 'logo_tmp')
os.makedirs(TMP, exist_ok=True)

W, H, K = 440, 236, 4   # final pixels; rendered K times larger, then pixelized

bpy.ops.wm.read_factory_settings(use_empty=True)
sc = bpy.context.scene
engines = [e.identifier for e in bpy.types.RenderSettings.bl_rna.properties['engine'].enum_items]
sc.render.engine = 'BLENDER_EEVEE_NEXT' if 'BLENDER_EEVEE_NEXT' in engines else 'BLENDER_EEVEE'
sc.render.film_transparent = True
sc.render.resolution_x = W * K
sc.render.resolution_y = H * K
sc.render.resolution_percentage = 100
sc.render.image_settings.file_format = 'PNG'
sc.render.image_settings.color_mode = 'RGBA'
sc.view_settings.view_transform = 'Standard'
sc.view_settings.look = 'None'
try:
    sc.eevee.taa_render_samples = 64
except Exception:
    pass

# a bright studio sky so the bevels and the chrome ball have something to reflect
world = bpy.data.worlds.new('w')
sc.world = world
world.use_nodes = True
wn = world.node_tree
bg = wn.nodes.get('Background')
tc = wn.nodes.new('ShaderNodeTexCoord')
sep = wn.nodes.new('ShaderNodeSeparateXYZ')
ramp = wn.nodes.new('ShaderNodeValToRGB')
ramp.color_ramp.elements[0].color = (0.08, 0.05, 0.16, 1)
ramp.color_ramp.elements[1].color = (2.2, 2.1, 2.4, 1)
ramp.color_ramp.elements[0].position = 0.25
wn.links.new(tc.outputs['Generated'], sep.inputs[0])
wn.links.new(sep.outputs['Z'], ramp.inputs[0])
wn.links.new(ramp.outputs[0], bg.inputs[0])

def light(name, rot, energy, color):
    ld = bpy.data.lights.new(name, 'SUN')
    ld.energy = energy
    ld.color = color
    ld.angle = math.radians(8)
    lo = bpy.data.objects.new(name, ld)
    lo.rotation_euler = rot
    sc.collection.objects.link(lo)
light('key', (math.radians(-35), math.radians(-30), 0), 4.0, (1.0, 0.96, 0.9))
light('rim', (math.radians(-110), math.radians(20), 0), 2.0, (0.6, 0.8, 1.0))

# camera: orthographic, looking slightly down at letters standing up, so the tops of the extrusions show
cam_data = bpy.data.cameras.new('cam')
cam_data.type = 'ORTHO'
cam_data.ortho_scale = 4.4
cam = bpy.data.objects.new('cam', cam_data)
sc.collection.objects.link(cam)
tilt = math.radians(14)
cam.location = (0, -10 * math.cos(tilt), 10 * math.sin(tilt) + 0.0)
cam.rotation_euler = (math.radians(90) - tilt, 0, 0)
sc.camera = cam

def gradient_mat(name, stops, side_hex):
    """face material: a vertical colour gradient (light at the top) in object space"""
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nt = m.node_tree
    p = next(n for n in nt.nodes if n.type == 'BSDF_PRINCIPLED')
    tco = nt.nodes.new('ShaderNodeTexCoord')
    sp = nt.nodes.new('ShaderNodeSeparateXYZ')
    cr = nt.nodes.new('ShaderNodeValToRGB')
    els = cr.color_ramp.elements
    els[0].position, els[0].color = 0.0, (*srgb(stops[-1]), 1)
    els[1].position, els[1].color = 1.0, (*srgb(stops[0]), 1)
    for k, h in enumerate(stops[1:-1][::-1]):
        e = els.new((k + 1) / (len(stops) - 1))
        e.color = (*srgb(h), 1)
    nt.links.new(tco.outputs['Generated'], sp.inputs[0])
    nt.links.new(sp.outputs['Y'], cr.inputs[0])
    nt.links.new(cr.outputs[0], p.inputs['Base Color'])
    # a little self-glow keeps the faces vivid whatever the lighting
    nt.links.new(cr.outputs[0], p.inputs['Emission Color'])
    p.inputs['Emission Strength'].default_value = 0.45
    p.inputs['Roughness'].default_value = 0.25
    p.inputs['Metallic'].default_value = 0.2
    side = mat(name + '_side', srgb(side_hex), 0.3, 0.35)
    return m, side

FONT = load_font('C:/Windows/Fonts/seguibli.ttf', 'C:/Windows/Fonts/ariblk.ttf', 'C:/Windows/Fonts/impact.ttf')

def word(name, body, size, x, z, face, side, shear=0.12, depth=0.16, offset=0.0, y=0.0, bevel=0.035):
    cu = bpy.data.curves.new(name, 'FONT')
    cu.body = body
    cu.font = FONT
    cu.size = size
    cu.shear = shear
    cu.extrude = depth
    cu.bevel_depth = bevel
    cu.offset = offset
    cu.bevel_resolution = 2
    cu.align_x = 'CENTER'
    cu.align_y = 'CENTER'
    cu.space_character = 0.87
    ob = bpy.data.objects.new(name, cu)
    sc.collection.objects.link(ob)
    ob.rotation_euler = (math.radians(90), 0, 0)
    ob.location = (x, y, z)
    bpy.context.view_layer.objects.active = ob
    ob.select_set(True)
    bpy.ops.object.convert(target='MESH')
    ob = bpy.context.view_layer.objects.active
    ob.data.materials.clear()
    ob.data.materials.append(face)
    ob.data.materials.append(side)
    # faces turned toward the viewer get the gradient, everything else the darker side colour
    bm = bmesh.new()
    bm.from_mesh(ob.data)
    for f in bm.faces:
        f.material_index = 0 if f.normal.z > 0.6 else 1
    bm.to_mesh(ob.data)
    bm.free()
    ob.select_set(False)
    return ob

pink_face, pink_side = gradient_mat('rogue', ['#ffb3d2', '#ff6aa8', '#e0337f', '#a81c66'], '#6b1048')
cyan_face, cyan_side = gradient_mat('ball', ['#a3ecff', '#49c6ec', '#1c95c8', '#12669a'], '#0f3f6b')
plate = mat('plate', srgb('#2e2063'), 0.2, 0.4)
plate_side = mat('plate_side', srgb('#120c2b'), 0.2, 0.5)
rim = mat('rim', srgb('#ffcc4d'), 0.7, 0.25)
for nm, body, size, x, z in (('ROGUE', 'ROGUE', 0.95, 0.0, 0.5), ('BALL', 'BALL', 1.35, -0.08, -0.27)):
    face, side = (pink_face, pink_side) if nm == 'ROGUE' else (cyan_face, cyan_side)
    word(nm, body, size, x, z, face, side, depth=0.18)
    # a dark backing slab with a gold rim behind each word, like a backglass logo
    word(nm + '_plate', body, size, x, z, plate, plate_side, depth=0.1, offset=0.055, y=0.12, bevel=0.01)
    word(nm + '_rim', body, size, x, z, rim, rim, depth=0.06, offset=0.08, y=0.2, bevel=0.0)

# the chrome pinball streaking in on a gold swoosh under the letters
chrome = mat('chrome', srgb('#eef0f7'), 0.65, 0.18)
gold = mat('gold', srgb('#ffcc4d'), 0.6, 0.3)
bx, bz = 1.5, -0.86
sphere('pinball', bx, -0.3, bz, 0.26, chrome, seg=48, rings=24)
sw = []
for i in range(60):
    t = i / 59
    x = -1.9 + t * (bx - 0.3 + 1.9)
    z = -1.06 + 0.16 * math.sin(t * math.pi * 0.9) + t * 0.2
    sw.append((x, -0.25, z))
tube('swoosh', sw, 0.045, gold, seg=10)
tube('swoosh2', [(x, y, z - 0.11) for x, y, z in sw[10:50]], 0.022, gold, seg=8)
# speed lines trailing the ball
white = mat('glint', srgb('#ffffff'), emit=srgb('#ffffff'), strength=3.0)
for k, dz in enumerate((-0.12, 0.0, 0.12)):
    tube('speed%d' % k, [(bx - 0.95 + abs(dz), -0.32, bz + dz), (bx - 0.36, -0.32, bz + dz)], 0.018, white, seg=6)

def sparkle(name, x, z, r):
    pts = []
    for i in range(8):
        a = math.pi * i / 4
        rr = r if i % 2 == 0 else r * 0.22
        pts.append((x + math.cos(a) * rr, -0.6, z + math.sin(a) * rr))
    return make(name, [(x, -0.6, z)] + pts, [(0, i + 1, (i + 1) % 8 + 1) for i in range(8)], white, recalc=False)
sparkle('spark1', -1.55, 0.8, 0.15)
sparkle('spark2', 1.25, 0.88, 0.1)
sparkle('spark3', 1.85, -0.45, 0.09)

# ---------------------------------------------------------------- render, ID pass, pixelize
idm = bpy.data.materials.new('id')
idm.use_nodes = True
nt = idm.node_tree
for n in list(nt.nodes):
    nt.nodes.remove(n)
info = nt.nodes.new('ShaderNodeObjectInfo')
def mnode(op, a, b=None):
    n = nt.nodes.new('ShaderNodeMath')
    n.operation = op
    if isinstance(a, (int, float)): n.inputs[0].default_value = a
    else: nt.links.new(a, n.inputs[0])
    if b is not None:
        if isinstance(b, (int, float)): n.inputs[1].default_value = b
        else: nt.links.new(b, n.inputs[1])
    return n.outputs[0]
idx = info.outputs['Object Index']
cc = nt.nodes.new('ShaderNodeCombineColor')
nt.links.new(mnode('DIVIDE', mnode('MODULO', idx, 32), 31), cc.inputs[0])
nt.links.new(mnode('DIVIDE', mnode('MODULO', mnode('FLOOR', mnode('DIVIDE', idx, 32)), 32), 31), cc.inputs[1])
nt.links.new(mnode('DIVIDE', mnode('FLOOR', mnode('DIVIDE', idx, 1024)), 31), cc.inputs[2])
em = nt.nodes.new('ShaderNodeEmission')
nt.links.new(cc.outputs[0], em.inputs[0])
o = nt.nodes.new('ShaderNodeOutputMaterial')
nt.links.new(em.outputs[0], o.inputs[0])

ramp_of = {'ROGUE': 'pink', 'BALL': 'cyan', 'pinball': 'steel', 'swoosh': 'gold', 'swoosh2': 'gold', 'ROGUE_plate': 'violet', 'BALL_plate': 'violet', 'ROGUE_rim': 'gold', 'BALL_rim': 'gold'}
names = pixel.RAMP_NAMES
for i, ob in enumerate([o for o in bpy.data.objects if o.type == 'MESH']):
    rn = ramp_of.get(ob.name, 'white' if ob.name.startswith(('speed', 'spark')) else 'steel')
    ob.pass_index = (i + 2) * 16 + names.index(rn)

def read_png(path):
    img = bpy.data.images.load(path, check_existing=False)
    w, h = img.size
    a = np.empty(w * h * 4, np.float32)
    img.pixels.foreach_get(a)
    bpy.data.images.remove(img)
    return a.reshape(h, w, 4)[::-1].copy()

def render(ids):
    path = os.path.join(TMP, 'logo' + ('_id' if ids else '') + '.png')
    sc.render.filepath = path
    if ids:
        bpy.context.view_layer.material_override = idm
        f = sc.render.filter_size
        sc.render.filter_size = 0.01
    bpy.ops.render.render(write_still=True)
    if ids:
        bpy.context.view_layer.material_override = None
        sc.render.filter_size = f
    px = read_png(path)
    return pixel.decode_ids(px) if ids else px

rgba = render(False)
ids, ramps = render(True)
art = pixel.pixelize(rgba, K, ids=ids, ramps=ramps, alpha='hard', outer_outline=True, inner_outline=True)
# a second, thicker dark outline makes it read over anything
solid = art[..., 3] > 0
ring = np.zeros_like(solid)
ring[1:, :] |= solid[:-1, :]; ring[:-1, :] |= solid[1:, :]; ring[:, 1:] |= solid[:, :-1]; ring[:, :-1] |= solid[:, 1:]
ring &= ~solid
art[ring] = (11, 10, 20, 255)
h, w = art.shape[:2]
img = bpy.data.images.new('out', w, h, alpha=True)
img.pixels.foreach_set((art[::-1].astype(np.float32) / 255).ravel())
img.filepath_raw = OUT
img.file_format = 'PNG'
img.save()
print('LOGO', w, h, flush=True)
