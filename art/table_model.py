# Builds the table in Blender from build/layout.json.
# Every surface the ball can reach is modeled at its collision size, and nothing that isn't solid in the physics
# stands up off the playfield where the ball can roll (no floor supports under ramps, no bulbs in open lanes).
# Returns object groups for the renderer: base (static), over (above the ball), and posable sprites.
import bpy, math
from geom import (mat, srgb, make, empty, prism, flat, sweep, cylinder, dome, sphere, torus, tube, box,
                  circle_pts, circles_hull, capsule_outline, nut, load_font, text_mesh, clip_convex)

def star_post(name, x, y, z0, z1, mm, ro=0.0034, ri=0.0024, n=8):
    pts = []
    for i in range(n * 2):
        a = math.pi * i / n
        rr = ro if i % 2 == 0 else ri
        pts.append((x + math.cos(a) * rr, y + math.sin(a) * rr))
    return prism(name, pts, z0, z1, mm)

def screw(name, x, y, z, mm):
    return dome(name, x, y, 0.0024, z, 0.0013, mm, seg=12, rings=3)

def build(L):
    R = L['ball_r']
    S = {}
    def M(name, hexc, metal=0.0, rough=0.5, emit=None, strength=0.0, alpha=1.0):
        return mat(name, srgb(hexc), metal, rough, srgb(emit) if emit else None, strength, alpha)

    m = {
        'pf':      M('playfield', '#2e2063', rough=0.4),
        'pf_dark': M('pf_dark', '#1e1545', rough=0.4),
        'pf_mid':  M('pf_mid', '#42308a', rough=0.4),
        'pf_pink': M('pf_pink', '#a81c66', rough=0.4),
        'pf_cyan': M('pf_cyan', '#12669a', rough=0.4),
        'pf_gold': M('pf_gold', '#b8650f', rough=0.4),
        'pf_red':  M('pf_red', '#74141f', rough=0.4),
        'pf_line': M('pf_line', '#c7c7d9', rough=0.4),
        'pf_rim':  M('pf_rim', '#120c2b', rough=0.4),
        'cab':     M('cabinet', '#1b1830', rough=0.6),
        'steel':   M('steel', '#9a99b3', metal=0.55, rough=0.4),
        'chrome':  M('chrome', '#c7c7d9', metal=1.0, rough=0.15),
        'rubber':  M('rubber', '#1b1830', rough=0.85),
        'rub_red': M('rubber_red', '#b8222f', rough=0.6),
        'flip':    M('flipper_body', '#eef0f7', rough=0.35),
        'skirt':   M('bumper_skirt', '#b8222f', rough=0.4),
        'b_body':  M('bumper_body', '#c7c7d9', rough=0.3),
        'b_base':  M('bumper_base', '#0b0a14', rough=0.5),
        'stripe':  M('stripe', '#eef0f7', rough=0.4),
        'black':   M('black', '#0b0a14', metal=0.3, rough=0.5),
        'wire':    M('wire', '#c7c7d9', metal=1.0, rough=0.2),
        'ramp':    M('ramp_plastic', '#a3ecff', rough=0.1),
        'plastic': M('plastic', '#e0337f', rough=0.25),
        'plastic_c': M('plastic_c', '#1c95c8', rough=0.25),
        'apron':   M('apron', '#2e2b47', metal=0.3, rough=0.45),
        'card':    M('apron_card', '#eef0f7', rough=0.6),
        'gi':      M('gi', '#fff1a8', emit='#ffcc4d', strength=6.0),
    }
    base, over, glass = [], [], []
    sprites = {}

    def B(o):
        base.append(o); return o
    def O(o):
        over.append(o); return o
    def G(o):
        glass.append(o); return o

    # ------------------------------------------------------------- playfield and its print
    outline = [p[:2] for p in L['outline']]
    B(prism('playfield', outline, -0.006, 0.0, m['pf']))
    z = [0.0002]
    inner = [(x * 0.985 + 0.019 * 0.015, y if y < 0.5 else 0.5 + (y - 0.5) * 0.99) for x, y in outline]
    def decal(name, poly, mm):
        poly = clip_convex(poly, inner)
        if len(poly) < 3:
            return None
        z[0] += 0.00002
        return B(flat(name, poly, z[0], mm))
    # sunburst behind the bumpers
    cx, cy = 0.058, 0.75
    for i in range(28):
        a0 = 2 * math.pi * i / 28
        a1 = a0 + 2 * math.pi / 56
        a1 = a0 + 2 * math.pi / 112
        pts = [(cx, cy), (cx + 0.3 * math.cos(a0), cy + 0.3 * math.sin(a0)), (cx + 0.3 * math.cos(a1), cy + 0.3 * math.sin(a1))]
        # clip to the playfield roughly by keeping rays short near the walls
        if i % 2 == 0:
            decal('sun_%d' % i, pts, m['pf_dark'])
    # lane colour bands
    decal('band_outL', [(-0.233, 0.04), (-0.201, 0.04), (-0.201, 0.272), (-0.233, 0.272)], m['pf_red'])
    decal('band_outR', [(0.201, 0.04), (0.233, 0.04), (0.233, 0.272), (0.201, 0.272)], m['pf_red'])
    decal('band_shoot', [(0.241, -0.01), (0.271, -0.01), (0.271, 0.79), (0.241, 0.79)], m['pf_dark'])
    decal('band_orbit', [(0.188, 0.42), (0.233, 0.42), (0.233, 0.66), (0.188, 0.66)], m['pf_cyan'])
    # red/white hazard chevrons down the outlanes
    for side in (-1, 1):
        x0b, x1b = (-0.233, -0.201) if side < 0 else (0.201, 0.233)
        for k in range(12):
            y0b = 0.05 + k * 0.018
            decal('haz_%d_%d' % (side, k), [(x0b, y0b), (x1b, y0b + 0.012), (x1b, y0b + 0.019), (x0b, y0b + 0.007)] if side < 0
                  else [(x0b, y0b + 0.012), (x1b, y0b), (x1b, y0b + 0.007), (x0b, y0b + 0.019)], m['pf_line'])
    # dot grid in the open lower playfield
    for gy in range(5):
        for gx in range(9):
            px, py = -0.09 + gx * 0.0225 + (0.011 if gy % 2 else 0), 0.43 + gy * 0.02
            if abs(px + 0.085) < 0.04 or (px > 0.0 and py > 0.49):
                continue
            decal('dot_%d_%d' % (gx, gy), circle_pts(px, py, 0.0022, 10), m['pf_mid'])
    suitfont = load_font('C:/Windows/Fonts/seguisym.ttf')
    for k, (sym, x, y, mm) in enumerate([('♠', -0.13, 0.62, 'pf_mid'), ('♥', 0.155, 0.44, 'pf_mid'), ('♦', -0.02, 0.665, 'pf_mid'), ('♣', 0.14, 0.62, 'pf_mid')]):
        z[0] += 0.00002
        B(text_mesh('suit_%d' % k, sym, 0.03, x, y, z[0], m[mm], font=suitfont, extrude=0.0))
    # shot arrows painted under the arrow inserts
    def arrow(name, x, y, l, w, mm):
        decal(name, [(x - w * 0.45, y - l * 0.4), (x + w * 0.45, y - l * 0.4), (x + w * 0.45, y + l * 0.35), (x + w, y + l * 0.35),
                     (x, y + l), (x - w, y + l * 0.35), (x - w * 0.45, y + l * 0.35)], mm)
    arrow('arrow_ramp', -0.085, 0.37, 0.05, 0.022, m['pf_pink'])
    arrow('arrow_orbit', 0.211, 0.37, 0.045, 0.019, m['pf_cyan'])
    # insert surrounds: dark rim and a white ring, like a real playfield cut-out
    for ins in L['inserts']:
        if ins['shape'] != 'circle':
            continue
        decal('rim_' + ins['id'], circle_pts(ins['x'], ins['y'], ins['r'] + 0.0035, 28), m['pf_line'])
        decal('rimd_' + ins['id'], circle_pts(ins['x'], ins['y'], ins['r'] + 0.0022, 28), m['pf_rim'])

    # inserts: unlit in the base, lit as sprites
    for ins in L['inserts']:
        col = '#%02x%02x%02x' % tuple(int(v * 255) for v in ins['color'])
        x, y, r = ins['x'], ins['y'], ins['r']
        rot = math.radians(ins.get('rot', 0))
        if ins['shape'] == 'arrow':
            loc = [(r, 0), (-r * 0.6, r * 0.75), (-r * 0.2, 0), (-r * 0.6, -r * 0.75)]
        elif ins['shape'] == 'chevron':
            loc = [(r * 0.5, 0), (-r * 0.5, r), (-r * 0.9, r), (0.1 * r, 0), (-r * 0.9, -r), (-r * 0.5, -r)]
        else:
            loc = [(math.cos(a) * r, math.sin(a) * r) for a in [2 * math.pi * i / 28 for i in range(28)]]
        c, s = math.cos(rot), math.sin(rot)
        poly = [(x + px * c - py * s, y + px * s + py * c) for px, py in loc]
        unlit = mat('ins_off_' + ins['id'], tuple(v * 0.3 for v in srgb(col)), rough=0.15)
        lit = mat('ins_on_' + ins['id'], srgb(col), rough=0.15, emit=srgb(col), strength=5.0)
        if ins['shape'] == 'chevron':
            faces = [(0, 1, 2, 3), (0, 3, 4, 5)]
            B(make('ins_' + ins['id'], [(p[0], p[1], 0.0006) for p in poly], faces, unlit, recalc=False))
            on = make('insl_' + ins['id'], [(p[0], p[1], 0.0007) for p in poly], faces, lit, recalc=False)
        else:
            B(flat('ins_' + ins['id'], poly, 0.0006, unlit))
            on = flat('insl_' + ins['id'], poly, 0.0007, lit)
        sprites['ins_' + ins['id']] = {'objs': [on], 'frames': 1, 'pose': lambda i: None, 'outline': False}

    # ------------------------------------------------------------- walls, guides, posts
    for i, w in enumerate(L['walls']):
        vis = w['vis']
        if vis == 'outer':
            B(sweep('wall_outer', w['pts'], w['r'], 0.0, w['pts'][0][3], m['cab']))
            B(sweep('wall_outer_cap', [p[:2] for p in w['pts']], w['r'] + 0.0015, w['pts'][0][3], w['pts'][0][3] + 0.004, m['steel'], round_top=True))
        elif vis == 'rail':
            B(sweep('guide_%d' % i, w['pts'], w['r'], 0.0, w['pts'][0][3], m['steel'], round_top=True))
            p0, p1 = w['pts'][0], w['pts'][-1]
            if math.hypot(p1[0] - p0[0], p1[1] - p0[1]) > 0.05:
                for k, t in enumerate((0.2, 0.8)):
                    B(screw('guide_%d_screw%d' % (i, k), p0[0] + (p1[0] - p0[0]) * t, p0[1] + (p1[1] - p0[1]) * t, w['pts'][0][3], m['chrome']))
    for i, p in enumerate(L['posts']):
        x, y, r, zt = p['x'], p['y'], p['r'], p['zt']
        if p['mat'] == 'rubber':
            B(star_post('post_%d' % i, x, y, 0, zt, m['chrome']))
            B(torus('post_ring_%d' % i, x, y, R, r - 0.0028, 0.0028, m['rubber']))
            B(nut('post_nut_%d' % i, x, y, zt, m['chrome']))
        else:
            B(cylinder('post_%d' % i, x, y, r, 0, zt, m['chrome'], seg=24))
            B(nut('post_nut_%d' % i, x, y, zt, m['chrome'], r=r * 0.7))

    # ------------------------------------------------------------- slingshots (rubber band + arm animate)
    for s in L['slings']:
        sid, pts, r = s['id'], [tuple(p) for p in s['pts']], s['r']
        a, c = pts[s['kick'][0]], pts[s['kick'][1]]
        gx = sum(p[0] for p in pts) / 3; gy = sum(p[1] for p in pts) / 3
        nx, ny = -(c[1] - a[1]), c[0] - a[0]
        nl = math.hypot(nx, ny); nx /= nl; ny /= nl
        if nx * ((a[0] + c[0]) / 2 - gx) + ny * ((a[1] + c[1]) / 2 - gy) < 0:
            nx, ny = -nx, -ny
        for j, p in enumerate(pts):
            B(cylinder('sling_%s_post%d' % (sid, j), p[0], p[1], 0.0028, 0, 0.046, m['chrome'], seg=12))
            B(nut('sling_%s_nut%d' % (sid, j), p[0], p[1], 0.046, m['chrome']))
        B(dome('sling_%s_bulb' % sid, gx, gy, 0.004, 0.0, 0.009, m['gi'], seg=12, rings=4))
        # the band in three states: rest, mid-kick, full kick (kicking side bowed outward)
        bands, arms = [], []
        for kf, bow in enumerate((0.0, 0.003, 0.006)):
            mid = ((a[0] + c[0]) / 2 + nx * bow, (a[1] + c[1]) / 2 + ny * bow)
            hullpts = circles_hull(pts + [mid], r - 0.0018, seg=24)
            band = sweep('sling_%s_band%d' % (sid, kf), hullpts, 0.0018, R - 0.0055, R + 0.0055, m['rubber'], closed=True)
            bands.append(band)
            ang = math.atan2(c[1] - a[1], c[0] - a[0])
            arms.append(box('sling_%s_arm%d' % (sid, kf), (a[0] + c[0]) / 2 - nx * (0.006 - bow), (a[1] + c[1]) / 2 - ny * (0.006 - bow),
                            0.004, 0.03, 0.004, 0.012, m['black'], rot=ang))
        glow = mat('sling_glow_' + sid, srgb('#ff6aa8'), emit=srgb('#ff6aa8'), strength=0.4)
        strip = sweep('sling_%s_strip' % sid, [(a[0] + nx * (r + 0.004), a[1] + ny * (r + 0.004)), (c[0] + nx * (r + 0.004), c[1] + ny * (r + 0.004))],
                      0.0014, 0.0005, 0.0012, glow)
        def pose_sling(i, bands=bands, arms=arms, glow=glow):
            kf, lit = i % 3, i // 3
            for k in range(3):
                bands[k].hide_render = k != kf
                arms[k].hide_render = k != kf
            p = next(n for n in glow.node_tree.nodes if n.type == 'BSDF_PRINCIPLED')
            p.inputs['Emission Strength'].default_value = 8.0 if lit else 0.4
        sprites['sling_' + sid] = {'objs': bands + arms + [strip], 'frames': 6, 'pose': pose_sling, 'outline': True}
        # printed clear plastic above (overlay), with a lit version for flashes
        grown = circles_hull(pts, r + 0.006, seg=16)
        pm = m['plastic'] if sid == 'L' else m['plastic_c']
        O(prism('plastic_' + sid, grown, 0.046, 0.0485, pm))
        sym = text_mesh('plastic_sym_' + sid, '♠' if sid == 'L' else '♥', 0.024, gx, gy + 0.004, 0.0487, m['stripe'],
                        font=load_font('C:/Windows/Fonts/seguisym.ttf'))
        O(sym)
        for j, p in enumerate(pts):
            O(screw('plastic_screw_%s%d' % (sid, j), p[0], p[1], 0.0485, m['chrome']))
        litm = mat('plastic_lit_' + sid, srgb('#ffb3d2' if sid == 'L' else '#a3ecff'), emit=srgb('#ffb3d2' if sid == 'L' else '#a3ecff'), strength=4.0)
        plit = prism('plastic_lit_' + sid, grown, 0.0486, 0.0487, litm)
        sprites['plastic_lit_' + sid] = {'objs': [plit], 'frames': 1, 'pose': lambda i: None, 'outline': False, 'layer': 'over'}

    # ------------------------------------------------------------- flippers
    for f in L['flippers']:
        fid = f['id']
        piv = empty('flipper_%s' % fid, loc=(f['x'], f['y'], 0))
        parts = [
            prism('flipper_%s_body' % fid, capsule_outline(f['len'], f['r0'] - 0.003, f['r1'] - 0.003), 0.002, f['h'], m['flip'], parent=piv),
            prism('flipper_%s_rubber' % fid, capsule_outline(f['len'], f['r0'], f['r1']), 0.007, 0.019, m['rub_red'], parent=piv),
            cylinder('flipper_%s_cap' % fid, 0, 0, 0.0065, f['h'], f['h'] + 0.002, m['chrome'], seg=24, parent=piv),
            nut('flipper_%s_bolt' % fid, 0, 0, f['h'] + 0.002, m['steel'], r=0.003, parent=piv),
            prism('flipper_%s_stripe' % fid, [(0.016, -0.0022), (f['len'] - 0.004, -0.0012), (f['len'] - 0.004, 0.0012), (0.016, 0.0022)],
                  f['h'], f['h'] + 0.0004, m['pf_pink'], parent=piv),
        ]
        rest, up = math.radians(f['rest']), math.radians(f['up'])
        def pose_flip(i, piv=piv, rest=rest, up=up):
            piv.rotation_euler = (0, 0, rest + (up - rest) * i / 11)
        sprites['flipper_' + fid] = {'objs': parts, 'frames': 12, 'pose': pose_flip, 'outline': True}

    # ------------------------------------------------------------- pop bumpers
    caps = {'b1': '#49c6ec', 'b2': '#ff6aa8', 'b3': '#ffcc4d'}
    for b in L['bumpers']:
        bid, x, y, r = b['id'], b['x'], b['y'], b['r']
        B(cylinder('bumper_%s_base' % bid, x, y, r - 0.001, 0, 0.003, m['b_base']))
        B(cylinder('bumper_%s_skirt' % bid, x, y, r, 0.003, 0.009, m['skirt'], r_top=r - 0.006))
        B(cylinder('bumper_%s_body' % bid, x, y, r * 0.72, 0.009, 0.033, m['b_body']))
        for k in range(3):
            a = 2 * math.pi * k / 3 + 0.5
            B(cylinder('bumper_%s_rod%d' % (bid, k), x + math.cos(a) * r * 0.82, y + math.sin(a) * r * 0.82, 0.0012, 0.009, 0.033, m['steel'], seg=8))
        ring = cylinder('bumper_%s_ring' % bid, 0, 0, r + 0.0005, 0.015, 0.0195, m['chrome'])
        ring.location = (x, y, 0)
        dark = {'b1': '#12669a', 'b2': '#a81c66', 'b3': '#b8650f'}[bid]
        capm = mat('bumper_cap_' + bid, srgb(dark), rough=0.25, emit=srgb(caps[bid]), strength=0.0)
        cap = cylinder('bumper_%s_cap' % bid, x, y, r + 0.002, 0.033, 0.038, capm)
        domeo = dome('bumper_%s_dome' % bid, x, y, r * 0.72, 0.038, 0.006, capm)
        logo = cylinder('bumper_%s_logo' % bid, x, y, r * 0.36, 0.0442, 0.0446, m['stripe'], seg=5)
        def pose_bump(i, ring=ring, capm=capm):
            drop = (0.0, 0.004, 0.008, 0.011)[i % 4]
            ring.location.z = -drop
            p = next(n for n in capm.node_tree.nodes if n.type == 'BSDF_PRINCIPLED')
            p.inputs['Emission Strength'].default_value = 9.0 if i >= 4 else 0.0
        sprites['bumper_' + bid] = {'objs': [ring, cap, domeo, logo], 'frames': 8, 'pose': pose_bump, 'outline': True, 'tall': True}

    # ------------------------------------------------------------- drop targets and standups
    for d in L['drops']:
        (ax, ay), (bx, by) = d['a'], d['b']
        cx, cy = (ax + bx) / 2, (ay + by) / 2
        w = math.hypot(bx - ax, by - ay)
        rot = math.atan2(by - ay, bx - ax)
        B(box('drop_%s_slot' % d['id'], cx, cy, 0.0001, w + 0.002, 2 * d['r'] + 0.003, 0.0002, m['black'], rot=rot))
        piv = empty('drop_%s' % d['id'], loc=(cx, cy, 0))
        dm = mat('drop_' + d['id'], srgb('#e89a1c'), rough=0.35)
        t = box('drop_%s_face' % d['id'], 0, 0, 0, w - 0.002, 2 * d['r'] - 0.001, d['zt'], dm, rot=rot, parent=piv)
        st = box('drop_%s_stripe' % d['id'], -math.sin(rot) * -d['r'], math.cos(rot) * -d['r'], 0.013, w * 0.7, 0.0008, 0.006, m['stripe'], rot=rot, parent=piv)
        def pose_drop(i, piv=piv, zt=d['zt']):
            piv.location.z = -zt * i / 5
        sprites['drop_' + d['id']] = {'objs': [t, st], 'frames': 6, 'pose': pose_drop, 'outline': True}
    for s in L['standups']:
        (ax, ay), (bx, by) = s['a'], s['b']
        cx, cy = (ax + bx) / 2, (ay + by) / 2
        w = math.hypot(bx - ax, by - ay)
        rot = math.atan2(by - ay, bx - ax)
        nx, ny = s['n']
        piv = empty('stand_%s' % s['id'], loc=(cx, cy, 0), rot=(0, 0, rot))
        sm = mat('stand_' + s['id'], srgb('#e0337f'), rough=0.35)
        t = box('stand_%s_face' % s['id'], 0, 0, 0.006, w, 2 * s['r'] - 0.002, s['zt'] - 0.006, sm, parent=piv)
        st = box('stand_%s_stripe' % s['id'], 0, -s['r'] * 0.97, 0.016, w * 0.6, 0.0008, 0.005, m['stripe'], parent=piv)
        post = cylinder('stand_%s_post' % s['id'], 0, 0.0005, 0.0022, 0, 0.007, m['black'], seg=10, parent=piv)
        def pose_stand(i, piv=piv, rot=rot):
            piv.rotation_euler = (( -0.24, -0.12, 0.0, 0.12, 0.24)[i], 0, rot)
        sprites['stand_' + s['id']] = {'objs': [t, st, post], 'frames': 5, 'pose': pose_stand, 'outline': True}

    # ------------------------------------------------------------- spinner, gate, rollover wires
    for sp in L['spinners']:
        (ax, ay), (bx, by) = sp['a'], sp['b']
        z0 = sp['z']
        cx, cy = (ax + bx) / 2, (ay + by) / 2
        piv = empty('spinner_%s' % sp['id'], loc=(cx, cy, z0))
        w = abs(bx - ax) - 0.005
        plate = box('spinner_%s_plate' % sp['id'], 0, 0, -0.022, w, 0.0016, 0.02, m['chrome'], parent=piv)
        decalm = mat('spinner_decal', srgb('#e0337f'), rough=0.4)
        dec = box('spinner_%s_decal' % sp['id'], 0, -0.0009, -0.019, w * 0.8, 0.0004, 0.014, decalm, parent=piv)
        def pose_spin(i, piv=piv):
            piv.rotation_euler = (2 * math.pi * i / 12, 0, 0)
        sprites['spinner_' + sp['id']] = {'objs': [plate, dec], 'frames': 12, 'pose': pose_spin, 'outline': True, 'layer': 'over'}
        for k, px in enumerate((ax, bx)):
            O(tube('spinner_%s_bracket%d' % (sp['id'], k), [(px, ay, 0), (px, ay, z0 + 0.004), (px + (0.002 if k == 0 else -0.002), ay, z0 + 0.004)], 0.0013, m['wire']))
        O(tube('spinner_%s_axle' % sp['id'], [(ax, ay, z0), (bx, by, z0)], 0.001, m['steel']))
    for g in L['gates']:
        (ax, ay), (bx, by) = g['a'], g['b']
        mxg = (ax + bx) / 2
        piv = empty('gate_%s' % g['id'], loc=(mxg, ay, 0.04))
        flap = tube('gate_%s_wire' % g['id'], [(ax - mxg + 0.002, 0, 0), (ax - mxg + 0.002, 0, -0.026), (bx - mxg - 0.002, 0, -0.026), (bx - mxg - 0.002, 0, 0)], 0.0011, m['wire'], seg=6, parent=piv)
        def pose_gate(i, piv=piv):
            piv.rotation_euler = (-1.2 * i / 4, 0, 0)
        sprites['gate_' + g['id']] = {'objs': [flap], 'frames': 5, 'pose': pose_gate, 'outline': True, 'layer': 'over'}
        for k, px in enumerate((ax, bx)):
            O(tube('gate_%s_bracket%d' % (g['id'], k), [(px, ay, 0), (px, ay, 0.041)], 0.0013, m['wire']))
        O(tube('gate_%s_axle' % g['id'], [(ax, ay, 0.04), (bx, by, 0.04)], 0.001, m['steel']))
    for ro in L['rollovers']:
        x, y = ro['x'], ro['y']
        B(box('roll_%s_slot' % ro['id'], x, y, 0.0001, 0.004, 0.026, 0.0002, m['black']))
        B(tube('roll_%s' % ro['id'], [(x, y - 0.012, 0.0), (x, y - 0.006, 0.004), (x, y, 0.0055), (x, y + 0.006, 0.004), (x, y + 0.012, 0.0)], 0.0012, m['wire'], seg=6))

    # ------------------------------------------------------------- ramp: clear plastic up-ramp, wire-form return
    for rp in L['ramps']:
        path, split, solid, hw = rp['path'], rp['split'], rp['solid_from'], rp['half_w']
        left, right = rp['left'], rp['right']
        def edge(i, lat, dz):
            p = path[i]
            a = path[max(0, i - 1)]; b = path[min(len(path) - 1, i + 1)]
            tx, ty = b[0] - a[0], b[1] - a[1]
            tl = math.hypot(tx, ty) or 1
            return (p[0] - ty / tl * lat, p[1] + tx / tl * lat, p[2] + dz)
        n = split + 1
        verts = []
        for i in range(n):
            verts += [edge(i, hw + 0.006, 0), edge(i, -(hw + 0.006), 0)]
        G(make('ramp_floor', verts, [(i * 2, i * 2 + 1, i * 2 + 3, i * 2 + 2) for i in range(split)], m['ramp'], recalc=False))
        G(sweep('ramp_wall_l', left[:n], 0.0015, 0, 0, m['ramp']))
        G(sweep('ramp_wall_r', right[:n], 0.0015, 0, 0, m['ramp']))
        stick = mat('ramp_sticker', srgb('#ff6aa8'), rough=0.4)
        for k, i in enumerate(range(3, split - 1, 3)):
            p = path[i]
            a = path[i - 1]; b2 = path[i + 1]
            tx, ty = b2[0] - a[0], b2[1] - a[1]
            tl = math.hypot(tx, ty) or 1
            tx, ty = tx / tl, ty / tl
            nx2, ny2 = -ty, tx
            hz = p[2] + 0.0006
            chev = [(0, 0.006), (0.012, -0.004), (0.012, -0.0005), (0, 0.0095), (-0.012, -0.0005), (-0.012, -0.004)]
            O(make('ramp_chev_%d' % k, [(p[0] + nx2 * cx2 + tx * cy2, p[1] + ny2 * cx2 + ty * cy2, hz) for cx2, cy2 in chev],
                   [(0, 1, 2, 3), (0, 3, 4, 5)], stick, recalc=False))
        O(tube('ramp_lip_l', [(p[0], p[1], p[3]) for p in left[:n]], 0.002, m['steel']))
        O(tube('ramp_lip_r', [(p[0], p[1], p[3]) for p in right[:n]], 0.002, m['steel']))
        B(box('ramp_flap', path[0][0], path[0][1] - 0.005, 0.0001, hw * 2 + 0.012, 0.012, 0.0006, m['steel']))
        # warm bulbs inside the closed tunnel under the clear ramp
        for k, i in enumerate((4, 9)):
            p = path[i]
            B(dome('ramp_bulb_%d' % k, p[0], p[1], 0.004, 0.0, 0.009, m['gi'], seg=12, rings=4))
        idx = list(range(split - 1, len(path)))
        lift = R - math.sqrt(R ** 2 - 0.0095 ** 2) - 0.0012
        for name, lat, dz in (('wl', 0.0095, lift), ('wr', -0.0095, lift), ('sl', hw + 0.003, 0.012), ('sr', -(hw + 0.003), 0.012), ('tl', hw + 0.003, 0.026), ('tr', -(hw + 0.003), 0.026)):
            O(tube('ramp_wire_' + name, [edge(i, lat, dz) for i in idx], 0.0013, m['wire'], seg=8))
        for k, i in enumerate(idx[1::3]):
            hoop = [edge(i, (hw + 0.003) * math.cos(a), 0.012 - 0.012 * math.sin(a)) for a in [math.pi * j / 10 for j in range(11)]]
            O(tube('ramp_hoop_%d' % k, hoop, 0.001, m['wire'], seg=6))
        # the wire-form hangs from brackets on the cabinet wall, never from the playfield
        for k, i in enumerate(idx[6::5]):
            p = edge(i, hw + 0.003, 0.026)
            if p[0] < -0.17 and i < solid:
                O(tube('ramp_hanger_%d' % k, [p, (-0.238, p[1], p[2] + 0.004)], 0.0013, m['steel']))
        # the solid descending end sits on sheet-metal skirts down to the playfield
        if solid < len(path):
            sk_l = [[p[0], p[1], 0, max(p[3] - 0.028, 0.004)] for p in left[solid:]]
            sk_r = [[p[0], p[1], 0, max(p[3] - 0.028, 0.004)] for p in right[solid:]]
            B(sweep('ramp_skirt_l', sk_l, 0.0015, 0, 0, m['black']))
            B(sweep('ramp_skirt_r', sk_r, 0.0015, 0, 0, m['black']))
        # the cap wall and the deflector under the return are real walls; shown as dark sheet metal
        for w in L['walls']:
            if w['vis'] == 'hidden':
                G(sweep('ramp_cap', w['pts'], w['r'], 0, 0, m['ramp']))

    tl_m = mat('lane_plastic', srgb('#e89a1c'), rough=0.25)
    O(prism('lane_plastic', [(-0.052, 0.958), (0.142, 0.958), (0.142, 0.984), (-0.052, 0.984)], 0.042, 0.0445, tl_m))
    numfont = load_font('C:/Windows/Fonts/bahnschrift.ttf', 'C:/Windows/Fonts/impact.ttf')
    for k, x in enumerate([-0.015, 0.045, 0.105]):
        O(text_mesh('lane_num_%d' % k, str(k + 1), 0.014, x, 0.971, 0.0447, m['pf_rim'], font=numfont, extrude=0.0))
    for k, x in enumerate([-0.045, 0.015, 0.075, 0.135]):
        O(screw('lane_screw_%d' % k, x, 0.971, 0.0445, m['chrome']))

    # ------------------------------------------------------------- apron, plunger
    ap = L['apron']
    x0a, x1a, y0a, y1a = ap['x0'], ap['x1'], ap['y0'], ap['y1']
    B(prism('apron', [(x0a, y0a), (x1a, y0a), (x1a, y1a - 0.014), (x1a - 0.035, y1a), (x0a + 0.035, y1a), (x0a, y1a - 0.014)], 0, 0.028, m['apron']))
    B(tube('apron_lip', [(x0a, y1a - 0.014, 0.028), (x0a + 0.035, y1a, 0.028), (x1a - 0.035, y1a, 0.028), (x1a, y1a - 0.014, 0.028)], 0.0016, m['steel']))
    for k, x in enumerate((-0.222, -0.05, 0.05, 0.222)):
        B(screw('apron_screw_%d' % k, x, 0.02, 0.028, m['chrome']))
    B(flat('apron_card_l', [(-0.205, 0.004), (-0.065, 0.004), (-0.065, 0.042), (-0.205, 0.042)], 0.0283, m['card']))
    B(flat('apron_card_r', [(0.065, 0.004), (0.205, 0.004), (0.205, 0.042), (0.065, 0.042)], 0.0283, m['card']))
    pl = L['plunger']
    piv = empty('plunger', loc=(pl['x'], 0, R))
    def cyl_y(name, y0, y1, r, mm):
        seg = 16
        verts, faces = [], []
        for i in range(seg):
            a = 2 * math.pi * i / seg
            verts += [(math.cos(a) * r, y0, math.sin(a) * r), (math.cos(a) * r, y1, math.sin(a) * r)]
        for i in range(seg):
            j = (i + 1) % seg
            faces.append((i * 2, j * 2, j * 2 + 1, i * 2 + 1))
        faces.append(tuple(i * 2 for i in range(seg)))
        faces.append(tuple(i * 2 + 1 for i in range(seg - 1, -1, -1)))
        return make(name, verts, faces, mm, smooth=lambda k: k < seg, parent=piv)
    pparts = [cyl_y('plunger_rod', -0.03, pl['y0'] - 0.004, 0.003, m['chrome']), cyl_y('plunger_tip', pl['y0'] - 0.007, pl['y0'], 0.0085, m['rubber'])]
    coil = [(0.0058 * math.cos(t * 0.8), -0.028 + t * 0.0011, 0.0058 * math.sin(t * 0.8)) for t in range(0, 60)]
    pparts.append(tube('plunger_spring', coil, 0.0009, m['steel'], seg=6, parent=piv))
    def pose_plunger(i, piv=piv, travel=pl['travel']):
        piv.location.y = -travel * i / 7
    sprites['plunger'] = {'objs': pparts, 'frames': 8, 'pose': pose_plunger, 'outline': True}

    # the ball itself, rendered alone
    ballm = mat('ball_chrome', srgb('#eef0f7'), metal=1.0, rough=0.16)
    ball = sphere('ball', 0.0, 0.3, R, R, ballm, seg=48, rings=24)
    sprites['ball'] = {'objs': [ball], 'frames': 1, 'pose': lambda i: None, 'outline': True, 'studio': True, 'anchor': (0.0, 0.3, R)}

    return {'base': base, 'over': over, 'glass': glass, 'sprites': sprites}
