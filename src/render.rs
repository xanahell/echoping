//! Отрисовка интерфейса в обычное окно: ячейки ratatui превращаются в пиксели.
//! Шрифт DejaVu Sans Mono (кириллица), блочные символы и рамки рисуются точно,
//! эмодзи берутся из встроенного атласа пиксельных спрайтов, изображения рисуются поверх сетки.
use std::collections::HashMap;
use fontdue::{Font, FontSettings};
use ratatui::{buffer::Buffer, style::{Color, Modifier}};
use unicode_width::UnicodeWidthStr;
use crate::{app::App, emoji};

static REG: &[u8] = include_bytes!("../assets/fonts/DejaVuSansMono.ttf");
static BOLD: &[u8] = include_bytes!("../assets/fonts/DejaVuSansMono-Bold.ttf");

pub const BG: u32 = 0x0f1115;
pub const FG: u32 = 0xd9dce1;

struct Glyph { xmin: i32, ymin: i32, w: usize, h: usize, cov: Vec<u8> }

pub struct Renderer {
    reg: Font, bold: Font,
    pub px: f32, pub cell_w: u32, pub cell_h: u32, ascent: i32,
    cache: HashMap<(char, bool), Glyph>,
    atlas: Vec<u8>, atlas_w: u32,
}

fn rgb(c: Color, default: u32) -> u32 {
    match c {
        Color::Reset => default, Color::Black => 0x000000, Color::Red => 0xe05555, Color::Green => 0x5fcf73, Color::Yellow => 0xe6c84f,
        Color::Blue => 0x5b8cf0, Color::Magenta => 0xc772d6, Color::Cyan => 0x4fc8d8, Color::Gray => 0xb4b9c4, Color::DarkGray => 0x6c727f,
        Color::LightRed => 0xff7a7a, Color::LightGreen => 0x84e89a, Color::LightYellow => 0xfff176, Color::LightBlue => 0x7db3ff,
        Color::LightMagenta => 0xe29bf0, Color::LightCyan => 0x7fe3f0, Color::White => 0xffffff,
        Color::Rgb(r, g, b) => ((r as u32) << 16) | ((g as u32) << 8) | b as u32,
        Color::Indexed(i) => { let v = (8 + (i as u32 % 24) * 10).min(255); (v << 16) | (v << 8) | v }
    }
}
fn mix(a: u32, b: u32, t: u32) -> u32 { // t: 0..=255 доля цвета a
    let ch = |s: u32| ((((a >> s) & 255) * t + ((b >> s) & 255) * (255 - t)) / 255) << s;
    ch(16) | ch(8) | ch(0)
}

impl Renderer {
    pub fn new(px: f32) -> Self {
        let reg = Font::from_bytes(REG, FontSettings::default()).expect("шрифт");
        let bold = Font::from_bytes(BOLD, FontSettings::default()).expect("шрифт");
        let atlas = image::load_from_memory(emoji::ATLAS).map(|i| i.to_rgba8());
        let (atlas_w, atlas) = match atlas { Ok(a) => (a.width(), a.into_raw()), Err(_) => (0, vec![]) };
        let mut r = Renderer { reg, bold, px, cell_w: 8, cell_h: 16, ascent: 12, cache: HashMap::new(), atlas, atlas_w };
        r.set_px(px);
        r
    }
    pub fn set_px(&mut self, px: f32) {
        self.px = px.clamp(8.0, 72.0);
        let lm = self.reg.horizontal_line_metrics(self.px).unwrap();
        self.cell_w = self.reg.metrics('M', self.px).advance_width.round().max(4.0) as u32;
        self.cell_h = ((lm.ascent - lm.descent + lm.line_gap).ceil() as u32).max((self.px * 1.25) as u32);
        self.ascent = lm.ascent.round() as i32 + ((self.cell_h as f32 - (lm.ascent - lm.descent)) / 2.0).max(0.0) as i32;
        self.cache.clear();
    }
    /// Сколько ячеек помещается в окно такого размера.
    pub fn grid(&self, w: u32, h: u32) -> (u16, u16) { (((w / self.cell_w).max(1)) as u16, ((h / self.cell_h).max(1)) as u16) }

    fn glyph(&mut self, ch: char, bold: bool) -> &Glyph {
        if !self.cache.contains_key(&(ch, bold)) {
            let f = if bold { &self.bold } else { &self.reg };
            let g = if f.lookup_glyph_index(ch) == 0 { Glyph { xmin: 0, ymin: 0, w: 0, h: 0, cov: vec![] } }
                else { let (m, cov) = f.rasterize(ch, self.px); Glyph { xmin: m.xmin, ymin: m.ymin, w: m.width, h: m.height, cov } };
            self.cache.insert((ch, bold), g);
        }
        &self.cache[&(ch, bold)]
    }

    /// Рисует весь кадр: ячейки, затем изображения и курсор.
    pub fn draw(&mut self, buf: &mut [u32], w: u32, h: u32, tb: &Buffer, app: &App, cursor: Option<(u16, u16)>, show_cursor: bool) {
        buf.fill(BG);
        let area = tb.area;
        for y in 0..area.height {
            let mut skip = 0u16;
            for x in 0..area.width {
                if skip > 0 { skip -= 1; continue; }
                let cell = tb.get(x, y);
                let sym = cell.symbol();
                let wcells = (UnicodeWidthStr::width(sym) as u32).max(1);
                let (mut fg, mut bg) = (rgb(cell.fg, FG), rgb(cell.bg, BG));
                if cell.modifier.contains(Modifier::REVERSED) { std::mem::swap(&mut fg, &mut bg); }
                if cell.modifier.contains(Modifier::DIM) { fg = mix(fg, bg, 140); }
                let (px0, py0, cw) = (x as u32 * self.cell_w, y as u32 * self.cell_h, wcells * self.cell_w);
                if bg != BG { fill(buf, w, h, px0, py0, cw, self.cell_h, bg); }
                let bold = cell.modifier.contains(Modifier::BOLD);
                self.draw_symbol(buf, w, h, sym, px0, py0, cw, fg, bold);
                if wcells > 1 { skip = (wcells - 1) as u16; }
            }
        }
        // настоящие изображения поверх ячеек
        for p in &app.placements {
            if let Some(img) = app.imgs.get(&p.key) {
                let (dx, dy) = (p.rect.x as u32 * self.cell_w, p.rect.y as u32 * self.cell_h);
                let (dw, dh) = (p.rect.width as u32 * self.cell_w, p.rect.height as u32 * self.cell_h);
                blit_image(buf, w, h, img.w, img.h, &img.rgb, dx, dy, dw, dh);
            }
        }
        if let (true, Some((cx, cy))) = (show_cursor, cursor) {
            fill(buf, w, h, cx as u32 * self.cell_w, cy as u32 * self.cell_h + 2, 2, self.cell_h.saturating_sub(4), FG);
        }
    }

    fn draw_symbol(&mut self, buf: &mut [u32], w: u32, h: u32, sym: &str, px0: u32, py0: u32, cw: u32, fg: u32, bold: bool) {
        let Some(ch) = sym.chars().next() else { return };
        if ch == ' ' { return; }
        let (ch_h, hh) = (self.cell_h, self.cell_h / 2);
        match ch {
            '▀' => return fill(buf, w, h, px0, py0, cw, hh, fg),
            '▄' => return fill(buf, w, h, px0, py0 + hh, cw, ch_h - hh, fg),
            '█' => return fill(buf, w, h, px0, py0, cw, ch_h, fg),
            '▌' => return fill(buf, w, h, px0, py0, cw / 2, ch_h, fg),
            '▐' => return fill(buf, w, h, px0 + cw / 2, py0, cw - cw / 2, ch_h, fg),
            '░' | '▒' | '▓' => {
                let t = match ch { '░' => 64, '▒' => 128, _ => 192 };
                for yy in py0..(py0 + ch_h).min(h) { for xx in px0..(px0 + cw).min(w) { let i = (yy * w + xx) as usize; buf[i] = mix(fg, buf[i], t); } }
                return;
            }
            '─' | '│' | '┌' | '┐' | '└' | '┘' | '├' | '┤' | '┬' | '┴' | '┼' => {
                let (l, r, u, d) = match ch { '─' => (1, 1, 0, 0), '│' => (0, 0, 1, 1), '┌' => (0, 1, 0, 1), '┐' => (1, 0, 0, 1), '└' => (0, 1, 1, 0), '┘' => (1, 0, 1, 0),
                    '├' => (0, 1, 1, 1), '┤' => (1, 0, 1, 1), '┬' => (1, 1, 0, 1), '┴' => (1, 1, 1, 0), _ => (1, 1, 1, 1) };
                let t = (ch_h / 14).max(1);
                let (cx, cy) = (px0 + cw / 2 - t / 2, py0 + ch_h / 2 - t / 2);
                if l == 1 { fill(buf, w, h, px0, cy, cx + t - px0, t, fg); }
                if r == 1 { fill(buf, w, h, cx, cy, px0 + cw - cx, t, fg); }
                if u == 1 { fill(buf, w, h, cx, py0, t, cy + t - py0, fg); }
                if d == 1 { fill(buf, w, h, cx, cy, t, py0 + ch_h - cy, fg); }
                return;
            }
            _ => {}
        }
        if let Some(i) = emoji::index_of(sym) { self.blit_sprite(buf, w, h, i, px0, py0, cw); return; }
        let (ascent, cell_h) = (self.ascent, self.cell_h as i32);
        let g = self.glyph(ch, bold);
        if g.w == 0 { return; }
        let gx = px0 as i32 + g.xmin;
        let gy = py0 as i32 + ascent - g.ymin - g.h as i32;
        let _ = cell_h;
        for yy in 0..g.h { for xx in 0..g.w {
            let a = g.cov[yy * g.w + xx] as u32;
            if a == 0 { continue; }
            let (x, y) = (gx + xx as i32, gy + yy as i32);
            if x < 0 || y < 0 || x as u32 >= w || y as u32 >= h { continue; }
            let i = (y as u32 * w + x as u32) as usize;
            buf[i] = mix(fg, buf[i], a);
        } }
    }

    fn blit_sprite(&self, buf: &mut [u32], w: u32, h: u32, idx: usize, px0: u32, py0: u32, cw: u32) {
        if self.atlas_w == 0 { return; }
        let s = emoji::CELL;
        let d = cw.min(self.cell_h).max(4);
        let (ox, oy) = (px0 + (cw - d) / 2, py0 + (self.cell_h - d) / 2);
        let (sx0, sy0) = ((idx as u32 % emoji::COLS) * s, (idx as u32 / emoji::COLS) * s);
        for dy in 0..d { for dx in 0..d {
            let (x, y) = (ox + dx, oy + dy);
            if x >= w || y >= h { continue; }
            // билинейная выборка из спрайта 24x24
            let (u, v) = (((dx as f32 + 0.5) * s as f32 / d as f32 - 0.5).clamp(0.0, s as f32 - 1.0), ((dy as f32 + 0.5) * s as f32 / d as f32 - 0.5).clamp(0.0, s as f32 - 1.0));
            let (x0, y0) = (u as u32, v as u32);
            let (x1, y1) = ((x0 + 1).min(s - 1), (y0 + 1).min(s - 1));
            let (fx, fy) = (u - x0 as f32, v - y0 as f32);
            let px = |xx: u32, yy: u32| -> [f32; 4] { let i = (((sy0 + yy) * self.atlas_w + sx0 + xx) * 4) as usize; [self.atlas[i] as f32, self.atlas[i + 1] as f32, self.atlas[i + 2] as f32, self.atlas[i + 3] as f32] };
            let (p00, p10, p01, p11) = (px(x0, y0), px(x1, y0), px(x0, y1), px(x1, y1));
            let mut c = [0f32; 4];
            for k in 0..4 { c[k] = (p00[k] * (1.0 - fx) + p10[k] * fx) * (1.0 - fy) + (p01[k] * (1.0 - fx) + p11[k] * fx) * fy; }
            let a = c[3] as u32;
            if a == 0 { continue; }
            let src = ((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32;
            let i = (y * w + x) as usize;
            buf[i] = mix(src, buf[i], a);
        } }
    }
}

fn fill(buf: &mut [u32], w: u32, h: u32, x: u32, y: u32, rw: u32, rh: u32, c: u32) {
    for yy in y..(y + rh).min(h) { let row = (yy * w) as usize; for xx in x..(x + rw).min(w) { buf[row + xx as usize] = c; } }
}

/// Изображение вписывается в прямоугольник с сохранением пропорций (билинейно).
fn blit_image(buf: &mut [u32], w: u32, h: u32, iw: u32, ih: u32, rgb: &[u8], dx: u32, dy: u32, dw: u32, dh: u32) {
    if iw == 0 || ih == 0 || dw == 0 || dh == 0 { return; }
    let s = (dw as f32 / iw as f32).min(dh as f32 / ih as f32);
    let (tw, th) = (((iw as f32 * s) as u32).max(1), ((ih as f32 * s) as u32).max(1));
    let (ox, oy) = (dx + (dw - tw) / 2, dy + (dh - th) / 2);
    for y in 0..th { for x in 0..tw {
        let (px, py) = (ox + x, oy + y);
        if px >= w || py >= h { continue; }
        let (u, v) = (((x as f32 + 0.5) / s - 0.5).clamp(0.0, iw as f32 - 1.0), ((y as f32 + 0.5) / s - 0.5).clamp(0.0, ih as f32 - 1.0));
        let (x0, y0) = (u as u32, v as u32);
        let (x1, y1) = ((x0 + 1).min(iw - 1), (y0 + 1).min(ih - 1));
        let (fx, fy) = (u - x0 as f32, v - y0 as f32);
        let g = |xx: u32, yy: u32, k: usize| rgb[((yy * iw + xx) * 3) as usize + k] as f32;
        let mut c = 0u32;
        for k in 0..3 {
            let a = g(x0, y0, k) * (1.0 - fx) + g(x1, y0, k) * fx;
            let b = g(x0, y1, k) * (1.0 - fx) + g(x1, y1, k) * fx;
            c = (c << 8) | (a * (1.0 - fy) + b * fy) as u32;
        }
        buf[(py * w + px) as usize] = c;
    } }
}
