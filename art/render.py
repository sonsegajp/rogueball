# Renders the table as pixel art for the game.
#   cargo run --release --bin lab -- --export-layout
#   blender --background --python art/render.py            (all)
#   blender --background --python art/render.py -- base    (only the named parts; e.g. base over flipper_L ball)
# Output: assets/table/*.png and assets/table/atlas.json
import bpy, sys, os, json, math
import numpy as np
from mathutils import Vector

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import pixel
import table_model
from geom import mat

ROOT = os.path.dirname(HERE)
OUT = os.path.join(ROOT, 'assets', 'table')
TMP = os.path.join(ROOT, 'build', 'render_tmp')
os.makedirs(OUT, exist_ok=True)
os.makedirs(TMP, exist_ok=True)
ONLY = sys.argv[sys.argv.index('--') + 1:] if '--' in sys.argv else []

L = json.load(open(os.path.join(ROOT, 'build', 'layout.json')))
bpy.ops.wm.read_factory_settings(use_empty=True)
model = table_model.build(L)

# ------------------------------------------------------------------ projection (shared with the game)
PHI = math.radians(20.0)         # camera tilt from straight down, toward the far end of the table
PPM = 1000.0                     # final pixels per meter (1 px = 1 mm)
K = 4                            # supersampling before pixelization
X0, X1 = -0.247, 0.285
SY0 = -0.02 * math.cos(PHI)
SY1 = 1.075 * math.cos(PHI) + 0.075 * math.sin(PHI)
W = int(round((X1 - X0) * PPM))
H = int(round((SY1 - SY0) * PPM))
X1 = X0 + W / PPM
SY0 = SY1 - H / PPM

def project(x, y, z):
    sy = y * math.cos(PHI) + z * math.sin(PHI)
    return (x - X0) * PPM, (SY1 - sy) * PPM

# ------------------------------------------------------------------ scene
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

cam_data = bpy.data.cameras.new('cam')
cam_data.type = 'ORTHO'
cam_data.ortho_scale = max(W, H) / PPM
cam_data.clip_end = 20
cam = bpy.data.objects.new('cam', cam_data)
sc.collection.objects.link(cam)
cx = (X0 + X1) / 2
syc = (SY0 + SY1) / 2
c0 = (cx, syc * math.cos(PHI), syc * math.sin(PHI))
cam.location = (c0[0], c0[1] - 4 * math.sin(PHI), c0[2] + 4 * math.cos(PHI))
cam.rotation_euler = (PHI, 0, 0)
sc.camera = cam

# lights: a soft key from the upper left, a cool fill, a warm rim from the far end
world = bpy.data.worlds.new('w')
sc.world = world
world.use_nodes = True
# a studio-ish gradient so metals have something bright to reflect
wn = world.node_tree
bg = wn.nodes.get('Background')
tc = wn.nodes.new('ShaderNodeTexCoord')
sep = wn.nodes.new('ShaderNodeSeparateXYZ')
ramp = wn.nodes.new('ShaderNodeValToRGB')
ramp.color_ramp.elements[0].color = (0.04, 0.035, 0.07, 1)
ramp.color_ramp.elements[1].color = (0.55, 0.52, 0.62, 1)
ramp.color_ramp.elements[0].position = 0.35
wn.links.new(tc.outputs['Generated'], sep.inputs[0])
wn.links.new(sep.outputs['Z'], ramp.inputs[0])
wn.links.new(ramp.outputs[0], bg.inputs[0])
bg.inputs[1].default_value = 1.0
def light(name, kind, loc, rot, energy, color, size=None):
    ld = bpy.data.lights.new(name, kind)
    ld.energy = energy
    ld.color = color
    if size is not None:
        if kind == 'SUN':
            ld.angle = size
        else:
            ld.size = size
    lo = bpy.data.objects.new(name, ld)
    lo.location = loc
    lo.rotation_euler = rot
    sc.collection.objects.link(lo)
    return lo
light('key', 'SUN', (0, 0, 2), (math.radians(28), math.radians(-24), 0), 4.2, (1.0, 0.95, 0.88), math.radians(6))
light('fill', 'SUN', (0, 0, 2), (math.radians(-35), math.radians(30), 0), 0.9, (0.6, 0.7, 1.0), math.radians(20))
light('rim', 'SUN', (0, 0, 2), (math.radians(-70), 0, 0), 1.0, (1.0, 0.75, 0.55), math.radians(10))

# ID material: flat emission encoding each object's pass_index, used for outlines
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
# 5 bits per channel so values survive the sRGB round trip through PNG
r = math_node('DIVIDE', math_node('MODULO', idx, 32), 31)
g = math_node('DIVIDE', math_node('MODULO', math_node('FLOOR', math_node('DIVIDE', idx, 32)), 32), 31)
b = math_node('DIVIDE', math_node('FLOOR', math_node('DIVIDE', idx, 1024)), 31)
comb = nt.nodes.new('ShaderNodeCombineColor')
nt.links.new(r, comb.inputs[0]); nt.links.new(g, comb.inputs[1]); nt.links.new(b, comb.inputs[2])
em = nt.nodes.new('ShaderNodeEmission')
nt.links.new(comb.outputs[0], em.inputs[0])
out = nt.nodes.new('ShaderNodeOutputMaterial')
nt.links.new(em.outputs[0], out.inputs[0])

# pass index = object id * 16 + material ramp. Ids are distinct per object, except the playfield print,
# which reads as one surface (no outlines between decals). The ramp comes from the material's colour.
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
    srgb_col = [c * 12.92 if c <= 0.0031308 else 1.055 * c ** (1 / 2.4) - 0.055 for c in col]
    if p.inputs['Metallic'].default_value > 0.5:
        return 0  # metals shade through the steel ramp whatever their tint
    return pixel.nearest_ramp(srgb_col)

all_objs = [o for o in bpy.data.objects if o.type in ('MESH', 'CURVE', 'FONT')]
for i, o in enumerate(all_objs):
    oid = i + 2
    if o.name == 'playfield' or o.name.split('_')[0] in ('sun', 'band', 'fan', 'arrow', 'rim', 'rimd', 'ins', 'haz', 'dot', 'fline', 'suit'):
        oid = 1
    o.pass_index = oid * 16 + material_ramp(o)

# ------------------------------------------------------------------ render helpers
def show_only(objs):
    keep = set(o.name for o in objs)
    for o in all_objs:
        o.hide_render = o.name not in keep

def read_png(path):
    img = bpy.data.images.load(path, check_existing=False)
    w, h = img.size
    a = np.empty(w * h * 4, np.float32)
    img.pixels.foreach_get(a)
    bpy.data.images.remove(img)
    return a.reshape(h, w, 4)[::-1].copy()

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
    if ids:
        return pixel.decode_ids(px)
    return px

def save_png(path, rgba8):
    h, w = rgba8.shape[:2]
    img = bpy.data.images.new('out', w, h, alpha=True)
    img.pixels.foreach_set((rgba8[::-1].astype(np.float32) / 255).ravel())
    img.filepath_raw = path
    img.file_format = 'PNG'
    img.save()
    bpy.data.images.remove(img)

atlas_path = os.path.join(OUT, 'atlas.json')
atlas = json.load(open(atlas_path)) if (ONLY and os.path.exists(atlas_path)) else {'sprites': {}}
atlas['proj'] = {'x0': X0, 'sy1': SY1, 'ppm': PPM, 'phi': PHI, 'w': W, 'h': H}

def want(name):
    return not ONLY or name in ONLY or any(name.startswith(p.rstrip('*')) for p in ONLY if p.endswith('*'))

# ------------------------------------------------------------------ layers
for layer, objs, mode, dither in (('base', model['base'], 'hard', 0.0), ('over', model['over'], 'hard', 0.0), ('glass', model['glass'], 'hard', 0.0)):
    if not want(layer):
        continue
    show_only(objs)
    rgba = render(layer)
    ids, ramps = render(layer, ids=True)
    art = pixel.pixelize(rgba, K, ids=ids, ramps=ramps, alpha=mode, dither=dither)
    save_png(os.path.join(OUT, layer + '.png'), art)
    atlas[layer] = {'file': layer + '.png', 'w': W, 'h': H}
    print('LAYER', layer, W, H, flush=True)

# ------------------------------------------------------------------ sprites
def sprite_bbox(spr):
    """pixel bounds of a sprite over all its frames, from the objects' projected bounding boxes"""
    xs, ys = [], []
    for i in range(spr['frames']):
        spr['pose'](i)
        bpy.context.view_layer.update()
        for o in spr['objs']:
            if o.type != 'MESH' or o.hide_render:
                continue
            for c in o.bound_box:
                w = o.matrix_world @ Vector(c)
                px, py = project(w.x, w.y, w.z)
                xs.append(px); ys.append(py)
    for o in spr['objs']:
        o.hide_render = False
    m = 4
    return (max(0, int(math.floor(min(xs))) - m), min(W, int(math.ceil(max(xs))) + m),
            max(0, int(math.floor(min(ys))) - m), min(H, int(math.ceil(max(ys))) + m))

def set_border(box):
    r = sc.render
    if box is None:
        r.use_border = False
        return
    x0, x1, y0, y1 = box
    r.use_border = True
    r.use_crop_to_border = True
    # half-pixel nudges keep Blender's float→int border maths on exact supersample boundaries
    r.border_min_x = (x0 * K + 0.5) / (W * K)
    r.border_max_x = (x1 * K + 0.5) / (W * K)
    r.border_min_y = ((H - y1) * K + 0.5) / (H * K)
    r.border_max_y = ((H - y0) * K + 0.5) / (H * K)

for name, spr in model['sprites'].items():
    if not want(name):
        continue
    # chrome reflects its surroundings: give the ball a bright studio sky instead of the dark scene world
    studio = spr.get('studio', False)
    ramp.color_ramp.elements[0].color = (0.32, 0.30, 0.40, 1) if studio else (0.04, 0.035, 0.07, 1)
    ramp.color_ramp.elements[1].color = (1.6, 1.55, 1.7, 1) if studio else (0.55, 0.52, 0.62, 1)
    show_only(spr['objs'])
    box = sprite_bbox(spr)
    show_only(spr['objs'])
    bx0, bx1, by0, by1 = box
    frames = []
    for i in range(spr['frames']):
        spr['pose'](i)
        bpy.context.view_layer.update()
        set_border(box)
        rgba = render('%s_%d' % (name, i))
        ids, ramps = render('%s_%d' % (name, i), ids=True)
        if rgba.shape[0] != (by1 - by0) * K or rgba.shape[1] != (bx1 - bx0) * K:
            # border maths didn't land on the grid: fall back to a full frame
            set_border(None)
            rgba = render('%s_%d' % (name, i))
            ids, ramps = render('%s_%d' % (name, i), ids=True)
            region = (0, 0)
        else:
            region = (by0, bx0)
        art = pixel.pixelize(rgba, K, ids=ids if spr.get('outline') else None, ramps=ramps, alpha='hard',
                             outer_outline=spr.get('outline', False), inner_outline=bool(spr.get('outline')))
        full = np.zeros((H, W, 4), np.uint8)
        full[region[0]:region[0] + art.shape[0], region[1]:region[1] + art.shape[1]] = art
        frames.append(full)
    set_border(None)
    spr['pose'](0)
    # crop all frames to their union bounding box
    alpha = np.zeros((H, W), bool)
    for f in frames:
        alpha |= f[..., 3] > 0
    ys, xs = np.nonzero(alpha)
    if len(xs) == 0:
        print('EMPTY sprite', name)
        continue
    x0, x1, y0, y1 = xs.min(), xs.max() + 1, ys.min(), ys.max() + 1
    strip = np.concatenate([f[y0:y1, x0:x1] for f in frames], axis=1)
    save_png(os.path.join(OUT, name + '.png'), strip)
    entry = {'file': name + '.png', 'frames': spr['frames'], 'w': int(x1 - x0), 'h': int(y1 - y0), 'x': int(x0), 'y': int(y0),
             'layer': spr.get('layer', 'floor')}
    if 'anchor' in spr:
        ax, ay = project(*spr['anchor'])
        entry['ax'] = ax - x0
        entry['ay'] = ay - y0
    atlas['sprites'][name] = entry
    print('SPRITE', name, entry['w'], entry['h'], spr['frames'], flush=True)

json.dump(atlas, open(atlas_path, 'w'), indent=1)
print('DONE', W, H)
