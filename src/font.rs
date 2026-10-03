//! Hand-drawn 5×7 pixel font, proportional spacing. All caps (lowercase maps to uppercase).

use macroquad::prelude::*;

const GLYPHS: &[(char, &str)] = &[
    ('A', ".###.|#...#|#...#|#####|#...#|#...#|#...#"),
    ('B', "####.|#...#|#...#|####.|#...#|#...#|####."),
    ('C', ".###.|#...#|#....|#....|#....|#...#|.###."),
    ('D', "####.|#...#|#...#|#...#|#...#|#...#|####."),
    ('E', "#####|#....|#....|####.|#....|#....|#####"),
    ('F', "#####|#....|#....|####.|#....|#....|#...."),
    ('G', ".###.|#...#|#....|#.###|#...#|#...#|.####"),
    ('H', "#...#|#...#|#...#|#####|#...#|#...#|#...#"),
    ('I', "###|.#.|.#.|.#.|.#.|.#.|###"),
    ('J', "..###|...#.|...#.|...#.|...#.|#..#.|.##.."),
    ('K', "#...#|#..#.|#.#..|##...|#.#..|#..#.|#...#"),
    ('L', "#....|#....|#....|#....|#....|#....|#####"),
    ('M', "#...#|##.##|#.#.#|#.#.#|#...#|#...#|#...#"),
    ('N', "#...#|#...#|##..#|#.#.#|#..##|#...#|#...#"),
    ('O', ".###.|#...#|#...#|#...#|#...#|#...#|.###."),
    ('P', "####.|#...#|#...#|####.|#....|#....|#...."),
    ('Q', ".###.|#...#|#...#|#...#|#.#.#|#..#.|.##.#"),
    ('R', "####.|#...#|#...#|####.|#.#..|#..#.|#...#"),
    ('S', ".####|#....|#....|.###.|....#|....#|####."),
    ('T', "#####|..#..|..#..|..#..|..#..|..#..|..#.."),
    ('U', "#...#|#...#|#...#|#...#|#...#|#...#|.###."),
    ('V', "#...#|#...#|#...#|#...#|#...#|.#.#.|..#.."),
    ('W', "#...#|#...#|#...#|#.#.#|#.#.#|#.#.#|.#.#."),
    ('X', "#...#|#...#|.#.#.|..#..|.#.#.|#...#|#...#"),
    ('Y', "#...#|#...#|.#.#.|..#..|..#..|..#..|..#.."),
    ('Z', "#####|....#|...#.|..#..|.#...|#....|#####"),
    ('0', ".###.|#...#|#..##|#.#.#|##..#|#...#|.###."),
    ('1', ".#.|##.|.#.|.#.|.#.|.#.|###"),
    ('2', ".###.|#...#|....#|...#.|..#..|.#...|#####"),
    ('3', "#####|...#.|..#..|...#.|....#|#...#|.###."),
    ('4', "...#.|..##.|.#.#.|#..#.|#####|...#.|...#."),
    ('5', "#####|#....|####.|....#|....#|#...#|.###."),
    ('6', "..##.|.#...|#....|####.|#...#|#...#|.###."),
    ('7', "#####|....#|...#.|..#..|.#...|.#...|.#..."),
    ('8', ".###.|#...#|#...#|.###.|#...#|#...#|.###."),
    ('9', ".###.|#...#|#...#|.####|....#|...#.|.##.."),
    (' ', "...|...|...|...|...|...|..."),
    ('!', "#|#|#|#|#|.|#"),
    ('"', "#.#|#.#|...|...|...|...|..."),
    ('#', ".#.#.|.#.#.|#####|.#.#.|#####|.#.#.|.#.#."),
    ('$', "..#..|.####|#.#..|.###.|..#.#|####.|..#.."),
    ('%', "##...|##..#|...#.|..#..|.#...|#..##|...##"),
    ('&', ".##..|#..#.|#.#..|.#...|#.#.#|#..#.|.##.#"),
    ('\'', "#|#|.|.|.|.|."),
    ('(', "..#|.#.|#..|#..|#..|.#.|..#"),
    (')', "#..|.#.|..#|..#|..#|.#.|#.."),
    ('*', ".....|..#..|#.#.#|.###.|#.#.#|..#..|....."),
    ('+', ".....|..#..|..#..|#####|..#..|..#..|....."),
    (',', "..|..|..|..|.#|.#|#."),
    ('-', "....|....|....|####|....|....|...."),
    ('.', "..|..|..|..|..|##|##"),
    ('/', "....#|...#.|...#.|..#..|.#...|.#...|#...."),
    (':', "..|##|##|..|##|##|.."),
    (';', "..|##|##|..|##|.#|#."),
    ('<', "...#|..#.|.#..|#...|.#..|..#.|...#"),
    ('=', "....|....|####|....|####|....|...."),
    ('>', "#...|.#..|..#.|...#|..#.|.#..|#..."),
    ('?', ".###.|#...#|....#|...#.|..#..|.....|..#.."),
    ('@', ".###.|#...#|#.###|#.#.#|#.###|#....|.###."),
    ('[', "##|#.|#.|#.|#.|#.|##"),
    (']', "##|.#|.#|.#|.#|.#|##"),
    ('_', "#####|.....|.....|.....|.....|.....|#####"),
    ('×', ".....|#...#|.#.#.|..#..|.#.#.|#...#|....."),
    ('·', ".|.|.|#|.|.|."),
    ('♠', "..#..|.###.|#####|#####|..#..|.###.|....."),
    ('♥', ".....|##.##|#####|#####|.###.|..#..|....."),
    ('♦', "..#..|.###.|#####|#####|.###.|..#..|....."),
    ('♣', "..#..|.###.|..#..|#####|#####|..#..|.###."),
    ('→', ".....|..#..|...#.|#####|...#.|..#..|....."),
    ('↑', "..#..|.###.|#.#.#|..#..|..#..|..#..|..#.."),
];

pub struct Font {
    tex: Texture2D,
    map: std::collections::HashMap<char, (u16, u8)>, // x offset in atlas, width
    bits: std::collections::HashMap<char, Vec<Vec<bool>>>,
}

impl Font {
    pub fn new() -> Font {
        let total: usize = GLYPHS.iter().map(|(_, g)| g.split('|').next().unwrap().len() + 1).sum();
        let mut img = Image::gen_image_color(total as u16, 7, Color::new(0.0, 0.0, 0.0, 0.0));
        let mut map = std::collections::HashMap::new();
        let mut bits = std::collections::HashMap::new();
        let mut x = 0usize;
        for (c, g) in GLYPHS {
            let rows: Vec<&str> = g.split('|').collect();
            let w = rows[0].len();
            for (y, row) in rows.iter().enumerate() {
                for (i, ch) in row.chars().enumerate() {
                    if ch == '#' {
                        img.set_pixel((x + i) as u32, y as u32, WHITE);
                    }
                }
            }
            map.insert(*c, (x as u16, w as u8));
            bits.insert(*c, rows.iter().map(|r| r.chars().map(|ch| ch == '#').collect()).collect());
            x += w + 1;
        }
        let tex = Texture2D::from_image(&img);
        tex.set_filter(FilterMode::Nearest);
        Font { tex, map, bits }
    }

    fn glyph(&self, c: char) -> (u16, u8) {
        let c = c.to_ascii_uppercase();
        *self.map.get(&c).or_else(|| self.map.get(&'?')).unwrap()
    }

    /// width in pixels of `s` at `scale`
    pub fn width(&self, s: &str, scale: f32) -> f32 {
        let mut w = 0.0;
        for c in s.chars() {
            w += (self.glyph(c).1 as f32 + 1.0) * scale;
        }
        (w - scale).max(0.0)
    }

    pub fn draw(&self, s: &str, x: f32, y: f32, scale: f32, color: Color) {
        let mut cx = x.round();
        let y = y.round();
        for c in s.chars() {
            let (gx, gw) = self.glyph(c);
            draw_texture_ex(&self.tex, cx, y, color, DrawTextureParams {
                source: Some(Rect::new(gx as f32, 0.0, gw as f32, 7.0)),
                dest_size: Some(vec2(gw as f32 * scale, 7.0 * scale)),
                ..Default::default()
            });
            cx += (gw as f32 + 1.0) * scale;
        }
    }

    /// glyph pixels (rows of lit flags) for custom renderers like the dot-matrix display
    pub fn bits(&self, c: char) -> &Vec<Vec<bool>> {
        let c = c.to_ascii_uppercase();
        self.bits.get(&c).or_else(|| self.bits.get(&'?')).unwrap()
    }

    /// text with a full 1-pixel dark outline and a drop shadow: reads on any background
    pub fn draw_outlined(&self, s: &str, x: f32, y: f32, scale: f32, color: Color) {
        let o = Color::new(0.04, 0.03, 0.08, color.a);
        let (x, y) = (x.round(), y.round());
        self.draw(s, x + scale, y + scale * 2.0, scale, o);
        for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0), (-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            self.draw(s, x + dx * scale.min(2.0), y + dy * scale.min(2.0), scale, o);
        }
        self.draw(s, x, y, scale, color);
    }

    pub fn draw_outlined_centered(&self, s: &str, cx: f32, y: f32, scale: f32, color: Color) {
        let w = self.width(s, scale);
        self.draw_outlined(s, (cx - w / 2.0).round(), y, scale, color);
    }

    /// text with a 1px dark shadow, the default for UI over art
    pub fn draw_shadow(&self, s: &str, x: f32, y: f32, scale: f32, color: Color) {
        self.draw(s, x + scale, y + scale, scale, Color::new(0.04, 0.03, 0.08, color.a));
        self.draw(s, x, y, scale, color);
    }

    pub fn draw_centered(&self, s: &str, cx: f32, y: f32, scale: f32, color: Color) {
        let w = self.width(s, scale);
        self.draw_shadow(s, (cx - w / 2.0).round(), y, scale, color);
    }

    /// word-wrap into lines no wider than `max_w`
    pub fn wrap(&self, s: &str, max_w: f32, scale: f32) -> Vec<String> {
        let mut lines = vec![];
        for para in s.split('\n') {
            let mut line = String::new();
            for word in para.split(' ') {
                let trial = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
                if self.width(&trial, scale) > max_w && !line.is_empty() {
                    lines.push(std::mem::take(&mut line));
                    line = word.to_string();
                } else {
                    line = trial;
                }
            }
            lines.push(line);
        }
        lines
    }
}
