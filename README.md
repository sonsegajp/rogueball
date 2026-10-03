# Rogueball

A pinball roguelike that runs in the browser. Every table in a run is procedurally generated and play-tested by a bot before you see it. It's written in Rust (macroquad) and compiled to WebAssembly. The pixel art is pre-rendered from Blender, and the soundtrack comes from a Game Boy Advance style chip synth written for the game.

**Play in the browser:** https://drewb583.github.io/rogueball/

## Controls

| Keyboard | Controller | Action |
| --- | --- | --- |
| Z / Left Shift / ← | LB / LT / d-pad left | Left flippers |
| / / Right Shift / → | RB / RT / d-pad right | Right flippers |
| Space (hold, release) | A (hold, release) | Launch |
| A D W | flick the left stick | Nudge (watch the tilt meter) |
| 1–9, or click | Y (first card) | Use a card |
| Esc | Start | Pause |
| Tab | Back | Run info |

Each ball scores **Points × Mult**. Beat each table's target before you run out of balls, then spend your money in the shop. Items and cards have no slot caps. The rack scrolls, and items trigger top to bottom; drag to reorder them, or right-click to sell in the shop. Making a ramp catches the ball on the flipper below it, and pressing flip shoots it.

## How it's built

- `src/tablegen.rs`: the table generator. The seed picks the width, the height, a theme, what each side wall carries (a ramp return, an orbit lane, an upper flipper or standups), the top lanes, and the toys in the middle. The flipper area is a fixed, proven module.
- `src/sim.rs`: the bot play-tester and the background table factory. It rejects layouts that trap the ball, can't reach their shots, or drain too fast.
- `src/physics.rs`: our own 1 kHz pinball physics.
- `src/rules.rs`: table rules over the layout's feature groups (lanes, banks, standups, ramps, orbits, combos, multiball, ball save, tilt).
- `src/bake.rs`: draws each generated layout's playfield, rails, ramps and apron in pixel art at load time.
- `art/kit.py`: renders the parts kit (flippers, bumpers, targets, slings, spinner, gate, plunger, posts) in Blender, at 1.0 and 0.8 px/mm.
- `art/cards.py`: one modeled scene per item and card.
- `art/pixel.py`: palette, per-material ramps and outlines.
- `src/chip.rs`, `src/music.rs`, `src/audio.rs`: the GBA-style sound chip (pulse, wave and noise channels, plus 8-bit PCM at 13 kHz with echo), the songs and the sound effects.
- `src/content.rs`, `src/run.rs`: items (rule data: trigger → condition → effect), cards, vouchers, bosses, tags, cabinets, and the scorer.
- `src/game.rs`, `src/draw.rs`, `src/ui.rs`: game flow, table drawing and UI.
- `src/platform.rs`, `web/`: saves in localStorage, the console log and the Gamepad API.

## Building

- **Web:** `./build_web.sh` builds `docs/`. Test it locally with `node web/serve.cjs docs 8123`.
- **Art:** `blender --background --python art/kit.py` and `blender --background --python art/cards.py`.
- **Lab:** `cargo run --release --bin lab -- --gen 40` generates 40 tables and reports the pass rate. `-- --diag N` plots seed N to `build/plot_N.png`.
- **Soundtrack preview:** `cargo run --release --bin jukebox` writes every song to `build/music/`.
