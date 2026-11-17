// Аватарки: цветной квадрат с инициалами или (если есть файл) уменьшенное фото
// из <config>/avatars/<имя>.png|jpg, нарисованное полублоками.
use std::path::Path;
use ratatui::{prelude::*};
use crate::settings::config_dir;

pub type Photo = [[(u8, u8, u8); 4]; 4];

const PAL: [(u8, u8, u8); 7] = [(214, 80, 80), (222, 140, 56), (150, 100, 214), (70, 160, 90), (50, 160, 175), (70, 125, 214), (204, 100, 146)];

fn hash(s: &str) -> usize { s.bytes().fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619)) as usize }

pub fn color_of(name: &str) -> Color { let c = PAL[hash(name) % PAL.len()]; Color::Rgb(c.0, c.1, c.2) }

pub fn initials(name: &str) -> String {
    // Только буквы и цифры шириной в одну клетку: иероглифы и эмодзи сломали бы выравнивание.
    let v: Vec<char> = name.split_whitespace().filter_map(|w| w.chars().next())
        .filter(|c| c.is_alphanumeric() && unicode_width::UnicodeWidthChar::width(*c) == Some(1)).take(2).collect();
    if v.is_empty() { "?".into() } else { v.iter().flat_map(|c| c.to_uppercase()).collect() }
}

pub fn load_photo(name: &str) -> Option<Photo> {
    let safe: String = name.chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect();
    let dir = config_dir().join("avatars");
    for ext in ["png", "jpg", "jpeg"] {
        let p = dir.join(format!("{safe}.{ext}"));
        if Path::new(&p).exists() {
            let img = image::open(&p).ok()?.resize_exact(4, 4, image::imageops::FilterType::Triangle).to_rgb8();
            let mut out = [[(0u8, 0u8, 0u8); 4]; 4];
            for y in 0..4 { for x in 0..4 { let px = img.get_pixel(x as u32, y as u32); out[y][x] = (px[0], px[1], px[2]); } }
            return Some(out);
        }
    }
    None
}

/// Две строки аватарки шириной 4 клетки.
pub fn rows(name: &str, photo: Option<&Photo>) -> [Vec<Span<'static>>; 2] {
    if let Some(p) = photo {
        let mk = |r: usize| (0..4).map(|c| {
            let t = p[2 * r][c]; let b = p[2 * r + 1][c];
            Span::styled("▀", Style::new().fg(Color::Rgb(t.0, t.1, t.2)).bg(Color::Rgb(b.0, b.1, b.2)))
        }).collect();
        return [mk(0), mk(1)];
    }
    let col = color_of(name);
    let ini = initials(name);
    let n = ini.chars().count();
    let txt = format!("{}{}{}", " ".repeat((4 - n) / 2), ini, " ".repeat(4 - n - (4 - n) / 2));
    [vec![Span::styled(txt, Style::new().fg(Color::White).bg(col).add_modifier(Modifier::BOLD))],
     vec![Span::styled("▀▀▀▀", Style::new().fg(col))]]
}
