// Отрисовка. Во время отрисовки заполняется список областей для мыши (Ctx.hits).
use ratatui::{prelude::*, widgets::*};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
use crate::{app::*, avatar, data::*, settings::{Kind as SKind, SECTIONS}};

const INFO_W: u16 = 30;

trait FrameExt { fn rw<W: Widget>(&mut self, w: W, r: Rect); }
impl FrameExt for Frame<'_> {
    /// render_widget с обрезкой по размеру окна: маленькое окно не должно приводить к аварии.
    fn rw<W: Widget>(&mut self, w: W, r: Rect) {
        let r = r.intersection(self.size());
        if r.width > 0 && r.height > 0 { self.render_widget(w, r); }
    }
}

pub struct Ctx { pub hits: Vec<(Rect, Hit)>, pub max_scroll: usize, pub list_cap: usize, pub placements: Vec<Placement>, pub cursor: Option<(u16, u16)> }

/// Место под настоящее пиксельное изображение (в оконном режиме рисуется поверх ячеек).
pub struct Placement { pub rect: Rect, pub key: i32 }

fn put_cursor(f: &mut Frame, cx: &mut Ctx, x: u16, y: u16) { f.set_cursor(x, y); cx.cursor = Some((x, y)); }

/// Рисует изображение полублоками (для терминала).
fn draw_halfblocks(f: &mut Frame, area: Rect, img: &Img) {
    let area = area.intersection(f.size());
    if area.width == 0 || area.height == 0 || img.w == 0 || img.h == 0 { return; }
    let s = (area.width as f32 / img.w as f32).min(area.height as f32 * 2.0 / img.h as f32);
    let dw = ((img.w as f32 * s).round() as u16).clamp(1, area.width);
    let dh = (((img.h as f32 * s) / 2.0).round() as u16).clamp(1, area.height);
    let (ox, oy) = (area.x + (area.width - dw) / 2, area.y + (area.height - dh) / 2);
    let px = |x: u32, y: u32| -> Color {
        let (sx, sy) = (((x as f32 + 0.5) / s) as u32, ((y as f32 + 0.5) / s) as u32);
        let (sx, sy) = (sx.min(img.w - 1), sy.min(img.h - 1));
        let i = ((sy * img.w + sx) * 3) as usize;
        Color::Rgb(img.rgb[i], img.rgb[i + 1], img.rgb[i + 2])
    };
    let buf = f.buffer_mut();
    for cy in 0..dh { for cxx in 0..dw {
        let cell = buf.get_mut(ox + cxx, oy + cy);
        cell.set_char('▀').set_fg(px(cxx as u32, cy as u32 * 2)).set_bg(px(cxx as u32, cy as u32 * 2 + 1));
    } }
}

/// Изображение: в окне как пиксели поверх ячеек, в терминале как полублоки.
fn draw_image(f: &mut Frame, a: &App, cx: &mut Ctx, area: Rect, key: i32) {
    if let Some(img) = a.imgs.get(&key) {
        if a.gui { cx.placements.push(Placement { rect: area, key }); } else { draw_halfblocks(f, area, img); }
    }
}

fn dim() -> Style { Style::new().fg(Color::DarkGray) }
fn bold() -> Style { Style::new().add_modifier(Modifier::BOLD) }
trait Wid { fn w(&self) -> usize; }
impl Wid for str { fn w(&self) -> usize { UnicodeWidthStr::width(self) } }

/// Обрезка до ширины w (в клетках терминала) без разрыва символов.
fn cut(s: &str, w: usize) -> String {
    let mut out = String::new(); let mut n = 0;
    for ch in s.chars() { let cw = UnicodeWidthChar::width(ch).unwrap_or(0); if n + cw > w { break; } out.push(ch); n += cw; }
    out
}
fn pad_r(s: &str, w: usize) -> String {
    let n = s.w();
    if n >= w { cut(s, w) } else { format!("{s}{}", " ".repeat(w - n)) }
}
fn trunc(s: &str, w: usize) -> String {
    if s.w() <= w { s.to_string() } else { format!("{}...", cut(s, w.saturating_sub(3))) }
}
fn wrap(s: &str, w: usize) -> Vec<String> {
    let mut out = vec![];
    for para in s.split('\n') {
        let mut line = String::new();
        for word in para.split_whitespace() {
            let mut word = word.to_string();
            while word.w() > w {
                if !line.is_empty() { out.push(std::mem::take(&mut line)); }
                let head = cut(&word, w.max(1));
                if head.is_empty() { break; }
                word = word[head.len()..].to_string();
                out.push(head);
            }
            let need = line.w() + if line.is_empty() { 0 } else { 1 } + word.w();
            if need > w && !line.is_empty() { out.push(std::mem::take(&mut line)); }
            if !line.is_empty() { line.push(' '); }
            line.push_str(&word);
        }
        out.push(line);
    }
    out
}
fn centered(area: Rect, w: u16, h: u16) -> Rect {
    Rect::new(area.x + area.width.saturating_sub(w) / 2, area.y + area.height.saturating_sub(h) / 2, w.min(area.width), h.min(area.height))
}
/// Строка из фрагментов; фрагменты с Some(Hit) становятся кликабельными.
fn segs(x: u16, y: u16, parts: Vec<(String, Style, Option<Hit>)>, cx: &mut Ctx) -> Line<'static> {
    let mut cur = x; let mut spans = vec![];
    for (t, s, h) in parts {
        let w = t.w() as u16;
        if let Some(h) = h { cx.hits.push((Rect::new(cur, y, w, 1), h)); }
        cur += w; spans.push(Span::styled(t, s));
    }
    Line::from(spans)
}
fn btn(t: &str, acc: Color, h: Hit) -> (String, Style, Option<Hit>) { (t.to_string(), Style::new().fg(acc), Some(h)) }
fn sp(t: &str) -> (String, Style, Option<Hit>) { (t.to_string(), Style::new(), None) }

pub fn draw(f: &mut Frame, app: &mut App) {
    app.ensure_photos();
    let mut cx = Ctx { hits: vec![], max_scroll: 0, list_cap: 1, placements: vec![], cursor: None };
    let area = f.size();
    {
        let a: &App = &*app;
        match a.screen {
            Screen::Login => draw_login(f, a, area, &mut cx),
            Screen::Main => match a.page {
                Page::Chats => {
                    let mut cols = vec![Constraint::Length(a.left_w), Constraint::Min(20)];
                    if a.info { cols.push(Constraint::Length(INFO_W)); }
                    let parts = Layout::horizontal(cols).split(area);
                    draw_left(f, a, parts[0], &mut cx);
                    draw_center(f, a, parts[1], &mut cx);
                    if a.info { draw_info(f, a, parts[2]); }
                    // Граница списка чатов: её можно тянуть мышью (регистрируется последней, чтобы иметь приоритет).
                    cx.hits.push((Rect::new(a.left_w.saturating_sub(1), 0, 1, area.height), Hit::Divider));
                }
                Page::Settings => draw_settings(f, a, area, &mut cx),
                Page::Sessions => draw_sessions(f, a, area, &mut cx),
                Page::Shop => draw_shop(f, a, area, &mut cx),
            },
        }
        if a.screen == Screen::Main {
            if a.emoji_open && a.page == Page::Chats { draw_emoji(f, a, area, &mut cx); }
            if let Some(v) = &a.viewer { draw_viewer(f, a, v, area, &mut cx); }
            if let Some(p) = &a.profile { draw_profile(f, a, p, area, &mut cx); }
            if let Some(m) = a.menu { draw_menu(f, a, m, area, &mut cx); }
            if let Some((ci, pos, m)) = a.chat_menu { draw_chat_menu(f, a, ci, pos, m, area, &mut cx); }
            if a.help { draw_help(f, area); }
        }
    }
    app.hits = cx.hits;
    app.placements = cx.placements;
    app.cursor = cx.cursor;
    app.max_scroll = cx.max_scroll;
    app.list_cap = cx.list_cap;
    app.scroll_up = app.scroll_up.min(app.max_scroll);
}

// ---------------- вход ----------------
fn draw_login(f: &mut Frame, a: &App, area: Rect, cx: &mut Ctx) {
    let acc = a.acc();
    let l = &a.login;
    let n = l.qr.len().max(33);
    let (qw, qh) = ((n + 4) as u16, ((n + 4 + 1) / 2) as u16);
    let (w, h) = match l.step { LStep::Qr => (qw + 3 + 46 + 4, qh.max(18) + 4), LStep::Api => (74, 20), _ => (62, 16) };
    let outer = centered(area, w, h);
    let blk = Block::bordered().border_style(dim()).title(Span::styled(if a.demo { " echoping: вход (ДЕМО) " } else { " echoping: вход в Telegram " }, bold().fg(acc)));
    let inner = blk.inner(outer).inner(&Margin { horizontal: 1, vertical: 0 });
    f.rw(blk, outer);
    let (x, y) = (inner.x, inner.y);
    let iw = inner.width;
    let red = Style::new().fg(Color::Red);
    let line_at = |f: &mut Frame, yy: u16, xx: u16, text: String, st: Style| f.rw(Paragraph::new(text).style(st), Rect::new(xx, yy, iw.saturating_sub(xx - x), 1));
    match l.step {
        LStep::Qr => {
            let dark = |xx: i32, yy: i32| -> bool {
                let off = (n - l.qr.len()) as i32 / 2;
                let (mx, my) = (xx - 2 - off, yy - 2 - off);
                let m = l.qr.len() as i32;
                mx >= 0 && my >= 0 && mx < m && my < m && l.qr[my as usize][mx as usize]
            };
            let col = |d: bool| if d { Color::Black } else { Color::White };
            let mut lines = vec![];
            for r in (0..(n + 4) as i32).step_by(2) {
                lines.push(Line::from((0..(n + 4) as i32).map(|c| Span::styled("▀", Style::new().fg(col(dark(c, r))).bg(col(dark(c, r + 1))))).collect::<Vec<_>>()));
            }
            f.rw(Paragraph::new(lines), Rect::new(x, y, qw, qh));
            if l.qr.is_empty() { f.rw(Paragraph::new(" получаю QR... ").style(Style::new().fg(Color::Black).bg(Color::White)), Rect::new(x + qw / 2 - 7, y + qh / 2, 15, 1)); }
            let tx = x + qw + 3;
            let t = vec![
                Line::from(Span::styled("Вход по QR-коду", bold())), Line::raw(""),
                Line::raw("1. Откройте Telegram на телефоне"), Line::raw("2. Настройки > Устройства"),
                Line::raw("3. Подключить устройство"), Line::raw("4. Наведите камеру на этот QR-код"), Line::raw(""),
                if a.demo { Line::from(Span::styled("ДЕМО: этот QR не рабочий.", Style::new().fg(Color::Yellow))) } else { Line::from(Span::styled(l.status.clone(), dim())) },
            ];
            f.rw(Paragraph::new(t), Rect::new(tx, y, 46, 8));
            if !l.err.is_empty() { f.rw(Paragraph::new(trunc(&l.err, 46)).style(red), Rect::new(tx, y + 9, 46, 1)); }
            f.rw(Paragraph::new(segs(tx, y + 11, vec![btn("[ Войти по номеру телефона ]", acc, Hit::LoginPhone)], cx)), Rect::new(tx, y + 11, 46, 1));
            if a.demo { f.rw(Paragraph::new(segs(tx, y + 13, vec![btn("[ Я отсканировал (демо) ]", acc, Hit::LoginScan)], cx)), Rect::new(tx, y + 13, 46, 1)); }
            else {
                f.rw(Paragraph::new(segs(tx, y + 13, vec![btn("[ Сменить api_id / api_hash ]", acc, Hit::LoginApi)], cx)), Rect::new(tx, y + 13, 46, 1));
                f.rw(Paragraph::new(segs(tx, y + 15, vec![btn("[ Попробовать без аккаунта (демо) ]", acc, Hit::DemoMode)], cx)), Rect::new(tx, y + 15, 46, 1));
            }
        }
        LStep::Wait => {
            line_at(f, y + 1, x, "Подключение".into(), bold());
            line_at(f, y + 3, x, l.status.clone(), Style::new());
            line_at(f, y + 5, x, trunc(&l.err, iw as usize), red);
            f.rw(Paragraph::new(segs(x, y + 8, vec![btn("[ Сменить api_id / api_hash ]", acc, Hit::LoginApi)], cx)), Rect::new(x, y + 8, iw, 1));
            f.rw(Paragraph::new(segs(x, y + 10, vec![btn("[ Попробовать без аккаунта (демо) ]", acc, Hit::DemoMode)], cx)), Rect::new(x, y + 10, iw, 1));
            line_at(f, y + 12, x, "Первый запуск TDLib может занять несколько секунд.".into(), dim());
        }
        LStep::Api => {
            line_at(f, y + 1, x, "Ключи Telegram API".into(), bold());
            line_at(f, y + 3, x, "Один раз получите их на my.telegram.org".into(), Style::new());
            line_at(f, y + 4, x, "(Log in > API development tools > Create application).".into(), dim());
            line_at(f, y + 5, x, "Ключи хранятся только в config.ini на этом компьютере.".into(), dim());
            let fw = (iw as usize).saturating_sub(16);
            for (k, (label, val)) in [("api_id", l.api_id.clone()), ("api_hash", l.api_hash.clone())].iter().enumerate() {
                let yy = y + 7 + 2 * k as u16;
                let st = if l.field == k { bold() } else { Style::new() };
                f.rw(Paragraph::new(Line::from(vec![Span::styled(pad_r(label, 10), dim()), Span::raw("[ "), Span::styled(pad_r(&cut(val, fw.saturating_sub(1)), fw), st), Span::raw(" ]")])), Rect::new(x, yy, iw, 1));
                cx.hits.push((Rect::new(x, yy, iw, 1), Hit::LoginApi));
                if l.field == k { put_cursor(f, cx, x + 12 + cut(val, fw.saturating_sub(1)).w() as u16, yy); }
            }
            line_at(f, y + 12, x, trunc(&l.err, iw as usize), red);
            f.rw(Paragraph::new(segs(x, y + 14, vec![btn("[ Продолжить ]", acc, Hit::LoginNext), sp("   "), btn("[ Без аккаунта (демо) ]", acc, Hit::DemoMode)], cx)), Rect::new(x, y + 14, iw, 1));
            line_at(f, y + 16, x, "Ключи нужны Telegram для любого клиента. Без них доступен только демо-режим.".into(), dim());
        }
        _ => {
            let (title, sub, field) = match l.step {
                LStep::Phone => ("Номер телефона", "Например: +31 6 12345678. Telegram пришлёт код.".to_string(), format!("+{}", l.phone.trim_start_matches('+'))),
                LStep::Code => ("Код подтверждения", if l.code_hint.is_empty() { "Введите 5 цифр из Telegram.".to_string() } else { l.code_hint.clone() },
                    (0..5).map(|i| l.code.chars().nth(i).map(|c| c.to_string()).unwrap_or("_".into())).collect::<Vec<_>>().join(" ")),
                _ => ("Облачный пароль", "Аккаунт защищён двухфакторной проверкой.".to_string(),
                    if l.show { l.pass.clone() } else { "*".repeat(l.pass.chars().count()) }),
            };
            f.rw(Paragraph::new(vec![Line::from(Span::styled(title, bold())), Line::from(Span::styled(trunc(&sub, iw as usize), dim()))]), Rect::new(x, y + 1, iw, 2));
            let fw = (iw as usize).saturating_sub(18);
            let shown: String = { let c: Vec<char> = field.chars().collect(); let skip = c.len().saturating_sub(fw.saturating_sub(1)); c[skip..].iter().collect() };
            let mut parts = vec![sp("[ "), (pad_r(&shown, fw), bold(), None), sp(" ]")];
            if l.step == LStep::Pass { parts.push(btn(if l.show { " [o] скрыть" } else { " [o] показать" }, acc, Hit::LoginEye)); }
            f.rw(Paragraph::new(segs(x, y + 4, parts, cx)), Rect::new(x, y + 4, iw, 1));
            put_cursor(f, cx, x + 2 + shown.w().min(fw) as u16, y + 4);
            if !l.err.is_empty() { line_at(f, y + 6, x, trunc(&l.err, iw as usize), red); }
            else if !l.status.is_empty() && !a.demo { line_at(f, y + 6, x, trunc(&l.status, iw as usize), dim()); }
            let next = if l.step == LStep::Pass { "[ Войти ]" } else { "[ Далее ]" };
            f.rw(Paragraph::new(segs(x, y + 8, vec![btn(next, acc, Hit::LoginNext), sp("   "), btn("[ Назад ]", acc, Hit::LoginBack)], cx)), Rect::new(x, y + 8, iw, 1));
            if l.step == LStep::Pass && !l.hint.is_empty() { line_at(f, y + 10, x, trunc(&format!("Подсказка: {}", l.hint), iw as usize), dim()); }
        }
    }
    let hint = match l.step {
        LStep::Qr => if a.demo { "Enter: я отсканировал (демо)   P: по номеру телефона   Ctrl+Q: выход" } else { "P: по номеру телефона   K: ключи API   Ctrl+Q: выход" },
        LStep::Api => "Tab: следующее поле   Enter: продолжить   Ctrl+Q: выход",
        LStep::Wait => "K: ключи API   Ctrl+Q: выход",
        LStep::Pass => "Enter: войти   Tab: показать пароль   Esc: назад   Ctrl+Q: выход",
        _ => "Enter: далее   Esc: назад   Ctrl+Q: выход",
    };
    f.rw(Paragraph::new(hint).style(dim()).alignment(Alignment::Center), Rect::new(area.x, area.y + area.height.saturating_sub(1), area.width, 1));
}

// ---------------- левая колонка ----------------
fn draw_left(f: &mut Frame, a: &App, area: Rect, cx: &mut Ctx) {
    let acc = a.acc();
    let blk = Block::new().borders(Borders::RIGHT).border_style(dim());
    let inner = blk.inner(area);
    f.rw(blk, area);
    let rows = Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1), Constraint::Min(2), Constraint::Length(1)]).split(inner);
    // поиск
    cx.hits.push((rows[0], Hit::Search));
    let s = if a.focus == Focus::Search { format!(" [/] {}_", a.search) }
        else if a.search.is_empty() { " [/] Поиск (Ctrl+K)".to_string() } else { format!(" [/] {}", a.search) };
    f.rw(Paragraph::new(s).style(if a.focus == Focus::Search { Style::new() } else { dim() }), rows[0]);
    // папки
    let mut parts = vec![sp(" ")];
    let names: [&str; 7] = match a.st.idx("folder_style") { 1 => ["Все", "Лич", "Груп", "Кан", "Бот", "★", "Арх"], 2 => ["Все", "@", "&", "#", "%", "★", "[A]"], _ => FOLDERS };
    for (i, n) in names.iter().enumerate() {
        let st = if i == a.folder { Style::new().fg(Color::Black).bg(acc) } else { dim() };
        parts.push((n.to_string(), st, Some(Hit::Folder(i)))); parts.push(sp(" "));
    }
    f.rw(Paragraph::new(segs(rows[1].x, rows[1].y, parts, cx)), rows[1]);
    f.rw(Paragraph::new("-".repeat(inner.width as usize)).style(dim()), rows[2]);
    // список
    let vis = a.visible();
    let lr = rows[3];
    cx.hits.push((lr, Hit::ListArea));
    let cap = (lr.height / 2).max(1) as usize;
    cx.list_cap = cap;
    let off = a.list_off.min(vis.len().saturating_sub(cap));
    let w = lr.width as usize;
    let av = a.st.on("avatars");
    let pre = if av { 6 } else { 3 };
    let mut l1: Vec<Line> = vec![];
    for (vi, &ci) in vis.iter().enumerate().skip(off).take(cap) {
        let c = &a.chats[ci];
        let selected = vi == a.sel;
        let bg = if selected { Style::new().bg(Color::Rgb(40, 40, 40)) } else { Style::new() };
        cx.hits.push((Rect::new(lr.x, lr.y + 2 * (vi - off) as u16, lr.width, 2), Hit::Chat(vi)));
        let time = c.msgs.last().map(|m| m.time.clone()).unwrap_or_default();
        let nw = w.saturating_sub(pre + time.w() + 1);
        let name_st = if c.unread > 0 || selected { bold() } else { Style::new() };
        let (mut r1, mut r2) = (vec![Span::styled(if selected { ">" } else { " " }, Style::new().fg(acc))], vec![Span::raw(" ")]);
        if av {
            let [t, b] = avatar::rows(&c.name, a.avatars.get(&c.id).or_else(|| a.photos.get(&c.name).and_then(|p| p.as_ref())));
            r1.extend(t); r1.push(Span::raw(" ")); r2.extend(b); r2.push(Span::raw(" "));
        } else {
            r1.push(Span::styled(format!("{} ", c.icon()), dim())); r2.push(Span::raw("  "));
        }
        r1.push(Span::styled(pad_r(&c.name, nw), name_st)); r1.push(Span::styled(format!(" {time}"), dim()));
        let ghost_hold = a.ghost() && c.unread > 0;
        let badge = if c.unread > 0 {
            Span::styled(format!("[{}]", c.unread), if c.muted || ghost_hold { dim() } else { Style::new().fg(Color::Black).bg(acc) })
        } else if c.pinned { Span::styled(" ^", dim()) } else if c.muted { Span::styled(" ~", dim()) } else { Span::raw("") };
        let bw = badge.content.w();
        let pw = w.saturating_sub(pre + bw);
        r2.push(Span::styled(pad_r(&trunc(&c.preview(), pw), pw), dim())); r2.push(badge);
        l1.push(Line::from(r1).style(bg)); l1.push(Line::from(r2).style(bg));
    }
    f.rw(Paragraph::new(l1), lr);
    // низ: меню, плеер, невидимка
    let gs = if a.ghost() { Style::new().fg(Color::Black).bg(acc) } else { dim() };
    let mut bparts = vec![sp(" "), btn("[=] Меню", acc, Hit::OpenSettings), sp(" "), ("[~]".to_string(), gs, Some(Hit::Ghost))];
    if !a.demo {
        bparts.push(sp("  "));
        for i in 0..a.accts.len() {
            let un = a.acct_unread(i);
            let label = if un > 0 { format!("{}({})", a.acct_slot(i), un.min(99)) } else { format!("{}", a.acct_slot(i)) };
            let st = if i == a.active { Style::new().fg(Color::Black).bg(acc) } else if un > 0 { bold().fg(acc) } else { dim() };
            bparts.push((label, st, Some(Hit::Acct(i)))); bparts.push(sp(" "));
        }
        if a.accts.len() < crate::app::MAX_ACCOUNTS { bparts.push(btn("[+]", acc, Hit::AcctAdd)); }
    }
    let bottom = segs(rows[4].x, rows[4].y, bparts, cx);
    f.rw(Paragraph::new(bottom), rows[4]);
}

// ---------------- центр ----------------
struct Bub { lines: Vec<Line<'static>>, ranges: Vec<(usize, usize)> }

fn build(chat: &Chat, w: usize, sel: Option<usize>, acc: Color, ticks: bool, voice: Option<(usize, f32)>, ascii: bool) -> Bub {
    let mut lines: Vec<Line> = vec![];
    let mut ranges = vec![];
    let maxw = (w * 7 / 10).max(24).min(w.saturating_sub(6));
    let mut prev_day = String::new();
    for (i, m) in chat.msgs.iter().enumerate() {
        if m.day != prev_day {
            let label = format!(" {} ", m.day);
            let side = w.saturating_sub(label.w()) / 2;
            lines.push(Line::from(Span::styled(format!("{}{}{}", "-".repeat(side), label, "-".repeat(w.saturating_sub(side + label.w()))), dim())));
            prev_day = m.day.clone();
        }
        let mut body: Vec<(String, Style)> = vec![];
        if !m.out && chat.kind != 'p' && chat.kind != 'b' { body.push((m.author.clone(), bold().fg(avatar::color_of(&m.author)))); }
        if let Some(r) = &m.reply { body.push((format!("| {}", trunc(r, maxw.saturating_sub(2))), dim())); }
        let hi = Style::new().fg(Color::Yellow);
        let unl = if !m.out && !m.listened { " (*)" } else { "" };
        match &m.kind {
            Kind::Image(n, s) => { body.push((format!("[IMG] {n} ({s})"), hi)); body.push(("[o] открыть  [s] сохранить".into(), dim())); }
            Kind::File(n, s) => { body.push((format!("[FILE] {n} ({s})"), hi)); body.push(("[o] открыть  [s] сохранить".into(), dim())); }
            Kind::Voice(d) => {
                let (playing, fr) = match voice { Some((vi, fr)) if vi == i => (true, fr), _ => (false, 0.0) };
                let fl = (fr * 12.0) as usize;
                body.push((format!("({}) {}{} {d}{unl}", if playing { "||" } else { ">" }, "#".repeat(fl.min(12)), ".".repeat(12 - fl.min(12))), hi));
            }
            Kind::Video(d) => { body.push((format!("(@) видеосообщение {d}{unl}"), hi)); body.push(("[o] открыть во внешнем плеере".into(), dim())); }
            Kind::Call(d) => body.push((format!("(c) звонок: {d}"), hi)),
            Kind::Text => {}
        }
        if !m.text.is_empty() { let txt = if ascii { crate::emoji::to_ascii(&m.text) } else { m.text.clone() }; for l in wrap(&txt, maxw) { body.push((l, Style::new())); } }
        let tk = if m.out && ticks { if m.read { " vv" } else { " v" } } else { "" };
        let foot = format!("{}{}{}", if m.edited { "изм. " } else { "" }, m.time, tk);
        let iw = body.iter().map(|(s, _)| s.w()).max().unwrap_or(0).max(foot.w()).min(maxw);
        let bst = if sel == Some(i) { Style::new().fg(Color::White).add_modifier(Modifier::BOLD) } else if m.out { Style::new().fg(acc) } else { dim() };
        let pad = if m.out { w.saturating_sub(iw + 4) } else { 0 };
        let start = lines.len();
        let edge = || Line::from(vec![Span::raw(" ".repeat(pad)), Span::styled(format!("+{}+", "-".repeat(iw + 2)), bst)]);
        lines.push(edge());
        for (s, st) in body.iter() {
            lines.push(Line::from(vec![Span::raw(" ".repeat(pad)), Span::styled("| ", bst), Span::styled(pad_r(s, iw), *st), Span::styled(" |", bst)]));
        }
        lines.push(Line::from(vec![Span::raw(" ".repeat(pad)), Span::styled("| ", bst),
            Span::raw(" ".repeat(iw.saturating_sub(foot.w()))), Span::styled(foot, dim()), Span::styled(" |", bst)]));
        lines.push(edge());
        ranges.push((start, lines.len()));
    }
    Bub { lines, ranges }
}

fn draw_center(f: &mut Frame, a: &App, area: Rect, cx: &mut Ctx) {
    let acc = a.acc();
    let banner = a.reply.is_some() || a.edit.is_some();
    let mini = false;
    let call = a.call.is_some();
    let rows = Layout::vertical([
        Constraint::Length(1), Constraint::Length(if mini { 1 } else { 0 }), Constraint::Length(1), Constraint::Length(1),
        Constraint::Min(3), Constraint::Length(if banner { 1 } else { 0 }), Constraint::Length(1), Constraint::Length(1), Constraint::Length(1),
        Constraint::Length(if call { 1 } else { 0 }),
    ]).split(area);
    if let Some(c) = &a.call {
        let r = rows[9];
        let txt = format!(" (c) {} {}звонок: {} ({})  ", if c.incoming { "Входящий" } else { "Исходящий" }, if c.video { "видео" } else { "" }, c.user, c.state);
        let line = segs(r.x, r.y, vec![(txt, Style::new().fg(Color::Black).bg(Color::Yellow), None), sp(" "), ("[ Отклонить F9 ]".to_string(), Style::new().fg(Color::Black).bg(Color::Red), Some(Hit::CallDecline)),
            ("  принять можно на телефоне: в консоли нет аудио-движка".to_string(), dim(), None)], cx);
        f.rw(Paragraph::new(line), r);
    }
    // вкладки: если места мало, неактивные сжимаются до номера, у активной остаётся имя и крестик; все вкладки кликабельны
    let cur = a.cur();
    let mut parts = vec![sp(" ")];
    let n = a.tabs.len();
    let avail = (rows[0].width as usize).saturating_sub(2);
    let dig = |i: usize| (i + 1).to_string().len();
    let full_w = |nw: usize| (0..n).map(|i| dig(i) + 1 + nw + 3).sum::<usize>();
    let name_w = (4..=14).rev().find(|&nw| full_w(nw) <= avail);
    let compact_w = |act_name: usize| (0..n).map(|i| if Some(a.tabs[i]) == cur { dig(i) + 1 + act_name + 3 } else { dig(i) + 1 }).sum::<usize>();
    // окно вкладок, если даже сжатые не помещаются
    let (start, end) = if name_w.is_some() || compact_w(6) <= avail || n == 0 { (0, n) } else {
        let ci = a.tabs.iter().position(|&t| Some(t) == cur).unwrap_or(0);
        let fit = (avail.saturating_sub(12) / 3).max(1);
        let s0 = ci.saturating_sub(fit / 2).min(n.saturating_sub(fit));
        (s0, (s0 + fit).min(n))
    };
    for i in start..end {
        let t = a.tabs[i];
        let active = Some(t) == cur;
        let st = if active { bold().fg(acc) } else { dim() };
        match name_w {
            Some(nw) => {
                parts.push((format!("{} {}", i + 1, trunc(&a.chats[t].name, nw)), st, Some(Hit::Tab(t))));
                parts.push((" x".to_string(), st, Some(Hit::TabClose(t)))); parts.push((" ".to_string(), dim(), None));
            }
            None if active => {
                let used: usize = (start..end).filter(|&k| Some(a.tabs[k]) != cur).map(|k| dig(k) + 1).sum();
                let nw = avail.saturating_sub(used + dig(i) + 4).clamp(3, 14);
                parts.push((format!("{} {}", i + 1, trunc(&a.chats[t].name, nw)), st, Some(Hit::Tab(t))));
                parts.push((" x".to_string(), st, Some(Hit::TabClose(t)))); parts.push((" ".to_string(), dim(), None));
            }
            None => { parts.push((format!("{}", i + 1), st, Some(Hit::Tab(t)))); parts.push((" ".to_string(), dim(), None)); }
        }
    }
    if end - start < n { parts.push((format!("({}-{}/{})", start + 1, end, n), dim(), None)); }
    if a.tabs.is_empty() { parts.push(("Enter или клик по чату откроет вкладку".to_string(), dim(), None)); }
    f.rw(Paragraph::new(segs(rows[0].x, rows[0].y, parts, cx)), rows[0]);
    // мини-плеер

    let Some(ci) = cur else {
        f.rw(Paragraph::new("Нет чатов").style(dim()).alignment(Alignment::Center), rows[4]);
        return;
    };
    let c = &a.chats[ci];
    // шапка
    let left = format!(" {}  ", c.name);
    let ghost = if a.ghost() { " [~] НЕВИДИМКА " } else { "" };
    let btns_w = 16usize;
    let gap = (area.width as usize).saturating_sub(left.w() + c.status.w() + ghost.w() + btns_w);
    let mut hp = vec![(left, bold(), Some(Hit::HdrName)), (c.status.clone(), dim(), Some(Hit::HdrName)), (" ".repeat(gap), Style::new(), None)];
    if a.ghost() { hp.push((ghost.to_string(), Style::new().fg(Color::Black).bg(acc), Some(Hit::Ghost))); }
    hp.extend([btn("[/]", acc, Hit::HdrSearch), sp(" "), btn("(c)", acc, Hit::HdrCall), sp(" "), btn("[i]", acc, Hit::HdrInfo), sp(" "), btn("[:]", acc, Hit::HdrMenu), sp(" ")]);
    f.rw(Paragraph::new(segs(rows[2].x, rows[2].y, hp, cx)), rows[2]);
    let sep = "-".repeat(area.width as usize);
    f.rw(Paragraph::new(sep.clone()).style(dim()), rows[3]);
    f.rw(Paragraph::new(sep).style(dim()), rows[6]);

    // сообщения, якорь снизу
    let mr = rows[4];
    let h = mr.height as usize;
    let voice = a.voice.and_then(|(vc, vi, t)| if vc == ci {
        let d = if let Kind::Voice(s) = &c.msgs[vi].kind { let mut p = s.split(':'); let m: f32 = p.next().and_then(|x| x.parse().ok()).unwrap_or(0.0); let s: f32 = p.next().and_then(|x| x.parse().ok()).unwrap_or(1.0); (m * 60.0 + s).max(1.0) } else { 1.0 };
        Some((vi, t.elapsed().as_secs_f32() / d))
    } else { None });
    let b = build(c, mr.width as usize, a.msg_sel, acc, a.st.on("ticks"), voice, a.st.on("emoji_replace") && !a.gui);
    let total = b.lines.len();
    cx.max_scroll = total.saturating_sub(h);
    let target = match a.msg_sel { Some(i) if i < b.ranges.len() => b.ranges[i].1, _ => total.saturating_sub(a.scroll_up.min(cx.max_scroll)) };
    let end = target.max(h.min(total)).min(total);
    let start = end.saturating_sub(h);
    let padtop = h.saturating_sub(end - start);
    let mut view: Vec<Line> = vec![Line::raw(""); padtop];
    view.extend(b.lines[start..end].iter().cloned());
    cx.hits.push((mr, Hit::MsgArea));
    for (i, &(s, e)) in b.ranges.iter().enumerate() {
        let (s2, e2) = (s.max(start), e.min(end));
        if s2 < e2 { cx.hits.push((Rect::new(mr.x, mr.y + (padtop + s2 - start) as u16, mr.width, (e2 - s2) as u16), Hit::Msg(i))); }
    }
    f.rw(Paragraph::new(view), mr);

    if banner {
        let t = if let Some(i) = a.edit { format!(" Правка: {}", trunc(&c.msgs[i].text, 60)) }
            else { let i = a.reply.unwrap(); format!(" Ответ {}: {}", c.msgs[i].author, trunc(&c.msgs[i].text, 60)) };
        let line = segs(rows[5].x, rows[5].y, vec![(t.clone(), Style::new().fg(acc), None), (" ".repeat((area.width as usize).saturating_sub(t.w() + 4)), Style::new(), None), btn("[x] ", acc, Hit::BannerX)], cx);
        f.rw(Paragraph::new(line), rows[5]);
    }
    // ввод
    let ir = rows[7];
    if !c.read_only { cx.hits.push((ir, Hit::InputBox)); }
    if c.read_only {
        f.rw(Paragraph::new("Вы не можете писать в этот канал (только чтение)").style(dim()).alignment(Alignment::Center), ir);
    } else {
    let sendable = !a.input.trim().is_empty();
    let inw = (area.width as usize).saturating_sub(4 + 9);
    let shown: String = { let n = a.input.w(); a.input.chars().skip(n.saturating_sub(inw.saturating_sub(1))).collect() };
    let shown: String = if let Some(p) = &a.attach { let t = format!("Файл: {p}"); let n = t.w(); let sk = n.saturating_sub(inw.saturating_sub(1)); t.chars().skip(sk).collect() } else { shown };
    let text = if a.attach.is_some() { (pad_r(&shown, inw), Style::new().fg(acc), Some(Hit::InputBox)) }
        else if a.input.is_empty() && a.focus != Focus::Input { (pad_r("Написать сообщение...", inw), dim(), Some(Hit::InputBox)) } else { (pad_r(&shown, inw), Style::new(), Some(Hit::InputBox)) };
    let line = segs(ir.x, ir.y, vec![btn("[@]", acc, Hit::Attach), sp(" "), text, sp(" "), btn(":)", acc, Hit::Emoji), sp("  "),
        if sendable { btn("[>]", acc, Hit::SendBtn) } else { btn("(o)", acc, Hit::MicBtn) }, sp(" ")], cx);
    f.rw(Paragraph::new(line), ir);
    if a.focus == Focus::Input && a.menu.is_none() && !a.help { put_cursor(f, cx, ir.x + 4 + shown.w() as u16, ir.y); }
    }
    // подсказки
    let hint = if a.attach.is_some() { "Путь к файлу или перетащите файл в окно  Enter отправить  Esc отмена" } else { match a.focus {
        Focus::Messages => "Enter меню  r ответ  e изменить  o открыть  s сохранить  d удалить",
        Focus::Input => "Enter отправить  Up правка  PgUp листать  Tab колонки  F1 справка",
        Focus::List => "Enter открыть  Alt+P закрепить  Alt+A архив  Ctrl+F файл  Ctrl+G призрак",
        Focus::Search => "Enter к списку  Esc сбросить",
    } };
    let nt = format!("{}  ОЗУ {}", a.notice, a.st.shown_rt("ram_now"));
    let gap = (area.width as usize).saturating_sub(hint.w() + nt.w() + 3);
    let hl = if nt.w() + hint.w() + 3 > area.width as usize { Line::from(Span::styled(format!(" {}", trunc(&nt, (area.width as usize).saturating_sub(2))), dim())) }
        else { Line::from(vec![Span::styled(format!(" {hint}"), dim()), Span::raw(" ".repeat(gap)), Span::styled(nt, dim())]) };
    f.rw(Paragraph::new(hl), rows[8]);
}

fn draw_info(f: &mut Frame, a: &App, area: Rect) {
    let blk = Block::new().borders(Borders::LEFT).border_style(dim());
    let inner = blk.inner(area);
    f.rw(blk, area);
    let Some(ci) = a.cur() else { return };
    let c = &a.chats[ci];
    let count = |f: fn(&Kind) -> bool| c.msgs.iter().filter(|m| f(&m.kind)).count();
    let t = vec![
        { let [t, _] = avatar::rows(&c.name, a.avatars.get(&c.id).or_else(|| a.photos.get(&c.name).and_then(|p| p.as_ref()))); let mut v = vec![Span::raw(" ")]; v.extend(t); v.push(Span::styled(format!(" {}", trunc(&c.name, 20)), bold())); Line::from(v) },
        { let [_, b] = avatar::rows(&c.name, a.avatars.get(&c.id).or_else(|| a.photos.get(&c.name).and_then(|p| p.as_ref()))); let mut v = vec![Span::raw(" ")]; v.extend(b); v.push(Span::styled(format!(" {}", trunc(&c.status, 20)), dim())); Line::from(v) }, Line::raw(""),
        Line::from(" [ Звонок ]  [ Видео ]"), Line::from(format!(" [ Звук: {} ]", if c.muted { "выкл" } else { "вкл" })), Line::raw(""),
        Line::from(Span::styled(" Общие файлы", bold())),
        Line::from(format!("  фото:       {}", count(|k| matches!(k, Kind::Image(..))))),
        Line::from(format!("  файлы:      {}", count(|k| matches!(k, Kind::File(..))))),
        Line::from(format!("  голосовые:  {}", count(|k| matches!(k, Kind::Voice(..))))),
        Line::from(format!("  видео:      {}", count(|k| matches!(k, Kind::Video(..))))),
        Line::from(format!("  звонки:     {}", count(|k| matches!(k, Kind::Call(..))))), Line::raw(""),
        Line::from(Span::styled(if a.ghost() { " [~] невидимка включена" } else { " невидимка выключена" }, dim())),
        Line::from(Span::styled(" F2 закрыть", dim())),
    ];
    f.rw(Paragraph::new(t), inner);
}

// ---------------- настройки ----------------
fn draw_settings(f: &mut Frame, a: &App, area: Rect, cx: &mut Ctx) {
    let acc = a.acc();
    let rows = Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Min(3), Constraint::Length(1)]).split(area);
    let hd = segs(rows[0].x, rows[0].y, vec![sp(" "), btn("[<] Назад", acc, Hit::Back), ("   Настройки".to_string(), bold(), None)], cx);
    f.rw(Paragraph::new(hd), rows[0]);
    f.rw(Paragraph::new("-".repeat(area.width as usize)).style(dim()), rows[1]);
    let cols = Layout::horizontal([Constraint::Length(34), Constraint::Min(20)]).split(rows[2]);
    // разделы
    let cap = cols[0].height as usize;
    let off = if a.set_sec >= cap { a.set_sec + 1 - cap } else { 0 };
    let mut ls = vec![];
    for (i, s) in SECTIONS.iter().enumerate().skip(off).take(cap) {
        cx.hits.push((Rect::new(cols[0].x, cols[0].y + (i - off) as u16, cols[0].width.saturating_sub(1), 1), Hit::SetSection(i)));
        let sel = i == a.set_sec;
        let st = if sel && a.set_col == 0 { Style::new().fg(Color::Black).bg(acc) } else if sel { bold().fg(acc) } else if s.custom { Style::new().fg(acc) } else { Style::new() };
        ls.push(Line::from(Span::styled(format!(" {}{}", if sel { "> " } else { "  " }, pad_r(s.title, 30)), st)));
    }
    f.rw(Paragraph::new(ls), cols[0]);
    // пункты
    let r = cols[1].inner(&Margin { horizontal: 2, vertical: 0 });
    cx.hits.push((r, Hit::SetArea));
    let sec = &SECTIONS[a.set_sec];
    if a.set_sec == 0 {
        let [t, b] = avatar::rows(&a.me_name, a.avatars.get(&-1));
        f.rw(Paragraph::new(Line::from(t)), Rect::new(r.x, r.y, 4, 1));
        f.rw(Paragraph::new(Line::from(b)), Rect::new(r.x, r.y + 1, 4, 1));
        let nm = if a.me_name.is_empty() { "Мой аккаунт".to_string() } else { a.me_name.clone() };
        f.rw(Paragraph::new(Line::from(vec![Span::styled(nm, bold().fg(acc)), Span::styled(if a.me_premium { "  * Premium" } else { "" }, Style::new().fg(Color::Yellow))])), Rect::new(r.x + 5, r.y, r.width.saturating_sub(5), 1));
    } else {
        f.rw(Paragraph::new(Line::from(Span::styled(sec.title, bold().fg(acc)))), Rect::new(r.x, r.y, r.width, 1));
    }
    let icap = (r.height as usize).saturating_sub(2);
    let ioff = if a.set_item >= icap { a.set_item + 1 - icap } else { 0 };
    let mut ls = vec![];
    for (i, it) in sec.items.iter().enumerate().skip(ioff).take(icap) {
        cx.hits.push((Rect::new(r.x, r.y + 2 + (i - ioff) as u16, r.width, 1), Hit::SetItem(i)));
        let sel = a.set_col == 1 && i == a.set_item;
        let val = if sel && a.set_edit.is_some() { format!("{}_", a.set_edit.as_ref().unwrap()) } else { a.st.shown(it) };
        let w = r.width as usize;
        let vw = val.w().min(w / 2);
        let lw = w.saturating_sub(vw + 1);
        let st = if sel { Style::new().fg(Color::Black).bg(acc) } else if matches!(it.kind, SKind::Info) { dim() } else { Style::new() };
        let vs = if sel { st } else { Style::new().fg(acc) };
        let val_t: String = if val.w() > vw { val.chars().skip(val.w() - vw).collect() } else { val };
        ls.push(Line::from(vec![Span::styled(pad_r(it.label, lw), st), Span::styled(" ", st), Span::styled(val_t, vs)]));
    }
    f.rw(Paragraph::new(ls), Rect::new(r.x, r.y + 2, r.width, r.height.saturating_sub(2)));
    let hint = if a.set_edit.is_some() { " Введите значение  Enter сохранить  Esc отмена" } else { " Up/Down выбор  Enter/клик изменить  Left/Right значение  ПКМ назад  Esc выход" };
    f.rw(Paragraph::new(hint).style(dim()), rows[3]);
}

// ---------------- оверлеи ----------------
fn draw_menu(f: &mut Frame, a: &App, sel: usize, area: Rect, cx: &mut Ctx) {
    let acc = a.acc();
    // Если меню не помещается, показываем окно из пунктов вокруг выбранного (прокрутка колесом мыши).
    let vis_n = (area.height.saturating_sub(2) as usize).clamp(1, MENU.len());
    let first = if sel >= vis_n { sel + 1 - vis_n } else { 0 };
    let (w, h) = (30u16, vis_n as u16 + 2);
    let r = match a.menu_pos {
        Some((x, y)) => Rect::new(x.min(area.width.saturating_sub(w)), y.min(area.height.saturating_sub(h)), w, h),
        None => centered(area, w, h),
    };
    f.rw(Clear, r);
    let lines: Vec<Line> = MENU.iter().enumerate().skip(first).take(vis_n).map(|(i, s)| {
        cx.hits.push((Rect::new(r.x + 1, r.y + 1 + (i - first) as u16, w - 2, 1), Hit::MenuItem(i)));
        if i == sel { Line::from(Span::styled(format!(" > {}", pad_r(s, 24)), Style::new().fg(Color::Black).bg(acc))) } else { Line::from(format!("   {s}")) }
    }).collect();
    let more = if MENU.len() > vis_n { format!(" {}/{} ", sel + 1, MENU.len()) } else { String::new() };
    f.rw(Paragraph::new(lines).block(Block::bordered().title(more)), r);
}

// ---------------- эмодзи ----------------
fn draw_emoji(f: &mut Frame, a: &App, area: Rect, cx: &mut Ctx) {
    let acc = a.acc();
    let left = a.left_w;
    let (w, h) = ((EMOJI_COLS * 3 + 2) as u16, 7u16);
    let r = Rect::new(left + 1, area.height.saturating_sub(2 + h).max(1), w.min(area.width.saturating_sub(left + 1)), h.min(area.height));
    f.rw(Clear, r);
    let rows_total = (crate::emoji::EMOJI.len() + EMOJI_COLS - 1) / EMOJI_COLS;
    let mut lines = vec![];
    for row in a.emoji_scroll..(a.emoji_scroll + 5).min(rows_total) {
        let mut sp_ = vec![];
        for col in 0..EMOJI_COLS {
            let idx = row * EMOJI_COLS + col;
            let Some((e, _)) = crate::emoji::EMOJI.get(idx) else { break };
            let (x, y) = (r.x + 1 + (col * 3) as u16, r.y + 1 + (row - a.emoji_scroll) as u16);
            cx.hits.push((Rect::new(x, y, 3, 1), Hit::EmojiPick(idx)));
            let st = if idx == a.emoji_sel { Style::new().bg(acc) } else { Style::new() };
            sp_.push(Span::styled(format!("{e} "), st));
        }
        lines.push(Line::from(sp_));
    }
    let title = format!(" Эмодзи {}/{} ", a.emoji_scroll + 1, rows_total);
    f.rw(Paragraph::new(lines).block(Block::bordered().title(title)), r);
}

// ---------------- просмотр изображения ----------------
fn draw_viewer(f: &mut Frame, a: &App, v: &Viewer, area: Rect, cx: &mut Ctx) {
    let acc = a.acc();
    let r = Rect::new(area.x + 2, area.y + 1, area.width.saturating_sub(4), area.height.saturating_sub(2));
    f.rw(Clear, r);
    let blk = Block::bordered().title(Span::styled(format!(" {} ", trunc(&v.title, r.width.saturating_sub(8) as usize)), bold().fg(acc)));
    let inner = blk.inner(r);
    f.rw(blk, r);
    let img_area = Rect::new(inner.x, inner.y, inner.width, inner.height.saturating_sub(2));
    cx.hits.push((r, Hit::ViewerClose));
    if v.loading { f.rw(Paragraph::new("Загрузка изображения...").style(dim()).alignment(Alignment::Center), Rect::new(img_area.x, img_area.y + img_area.height / 2, img_area.width, 1)); }
    else if a.imgs.contains_key(&v.key) { draw_image(f, a, cx, img_area, v.key); }
    else { f.rw(Paragraph::new("Не удалось показать изображение").style(dim()).alignment(Alignment::Center), Rect::new(img_area.x, img_area.y + img_area.height / 2, img_area.width, 1)); }
    let by = inner.y + inner.height.saturating_sub(1);
    let mut parts = vec![sp(" "), btn("[ Закрыть Esc ]", acc, Hit::ViewerClose)];
    if v.ext.is_some() { parts.push(sp("  ")); parts.push(btn("[ Открыть в системе O ]", acc, Hit::ViewerOpen)); }
    f.rw(Paragraph::new(segs(inner.x, by, parts, cx)), Rect::new(inner.x, by, inner.width, 1));
}

// ---------------- профиль ----------------
fn draw_profile(f: &mut Frame, a: &App, p: &Profile, area: Rect, cx: &mut Ctx) {
    let acc = a.acc();
    let (w, h) = (66u16.min(area.width), 20u16.min(area.height));
    let r = centered(area, w, h);
    f.rw(Clear, r);
    let blk = Block::bordered().title(Span::styled(" Профиль ", bold().fg(acc)));
    let inner = blk.inner(r).inner(&Margin { horizontal: 1, vertical: 0 });
    f.rw(blk, r);
    cx.hits.push((r, Hit::ProfileClose));
    let has_photo = p.photo.map(|k| a.imgs.contains_key(&k)).unwrap_or(false);
    let ph_w = if has_photo { 18u16 } else { 0 };
    if let Some(k) = p.photo.filter(|_| has_photo) { draw_image(f, a, cx, Rect::new(inner.x, inner.y, ph_w - 2, (ph_w - 2) / 2 + 1), k); }
    let tx = inner.x + ph_w;
    let tw = inner.width.saturating_sub(ph_w) as usize;
    let mut lines = vec![Line::from(Span::styled(trunc(&p.title, tw), bold().fg(acc))), Line::raw("")];
    for l in &p.lines { for w_ in wrap(l, tw.max(10)) { lines.push(Line::from(w_)); } }
    f.rw(Paragraph::new(lines), Rect::new(tx, inner.y, inner.width.saturating_sub(ph_w), inner.height.saturating_sub(1)));
    f.rw(Paragraph::new("Esc или клик: закрыть").style(dim()), Rect::new(inner.x, inner.y + inner.height.saturating_sub(1), inner.width, 1));
}

fn draw_help(f: &mut Frame, area: Rect) {
    let rows = [
        "Мышь: клик по всему, колесо - прокрутка, ПКМ - меню", "Tab            список <-> ввод", "Ctrl+K         поиск чатов", "Ctrl+Up/Down   сменить чат",
        "Left/Right     папки (в списке)", "Enter          открыть вкладку", "Alt+1..9       перейти во вкладку", "Ctrl+W         закрыть вкладку",
        "Up (пустой ввод) править последнее", "PgUp           листать сообщения", "Ctrl+G         режим невидимки", "Ctrl+O         настройки",
        "F3             плеер (F6/F7/F8 пред/пауза/след)", "F2             панель информации", "Ctrl+Q         выход",
    ];
    let r = centered(area, 56, rows.len() as u16 + 4);
    f.rw(Clear, r);
    let mut l = vec![Line::from(Span::styled(" Справка", bold())), Line::raw("")];
    l.extend(rows.iter().map(|s| Line::from(format!(" {s}"))));
    f.rw(Paragraph::new(l).block(Block::bordered()), r);
}

// ---------------- меню чата (ПКМ по списку) ----------------
fn draw_chat_menu(f: &mut Frame, a: &App, ci: usize, pos: (u16, u16), sel: usize, area: Rect, cx: &mut Ctx) {
    let acc = a.acc();
    let items = a.chat_menu_items(ci);
    let (w, h) = (30u16, items.len() as u16 + 2);
    let r = Rect::new(pos.0.min(area.width.saturating_sub(w)), pos.1.min(area.height.saturating_sub(h)), w, h);
    f.rw(Clear, r);
    let lines: Vec<Line> = items.iter().enumerate().map(|(i, t)| {
        cx.hits.push((Rect::new(r.x + 1, r.y + 1 + i as u16, w - 2, 1), Hit::ChatMenuItem(i)));
        if i == sel { Line::from(Span::styled(format!(" > {}", pad_r(t, 24)), Style::new().fg(Color::Black).bg(acc))) } else { Line::from(format!("   {t}")) }
    }).collect();
    f.rw(Paragraph::new(lines).block(Block::bordered()), r);
}

// ---------------- активные сеансы ----------------
fn draw_sessions(f: &mut Frame, a: &App, area: Rect, cx: &mut Ctx) {
    let acc = a.acc();
    let rows = Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Min(4), Constraint::Length(1), Constraint::Length(2), Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)]).split(area);
    f.rw(Paragraph::new(segs(rows[0].x, rows[0].y, vec![sp(" "), btn("[<] Назад", acc, Hit::Back), ("   Активные сеансы".to_string(), bold(), None)], cx)), rows[0]);
    f.rw(Paragraph::new("-".repeat(area.width as usize)).style(dim()), rows[1]);
    let lr = rows[2];
    if a.sessions.is_empty() { f.rw(Paragraph::new(" Загрузка списка сеансов...").style(dim()), lr); }
    let cap = (lr.height / 2).max(1) as usize;
    let off = if a.ses_sel >= cap { a.ses_sel + 1 - cap } else { 0 };
    let mut ls = vec![];
    for (i, sn) in a.sessions.iter().enumerate().skip(off).take(cap) {
        cx.hits.push((Rect::new(lr.x, lr.y + 2 * (i - off) as u16, lr.width, 2), Hit::SesRow(i)));
        let bg = if i == a.ses_sel { Style::new().bg(Color::Rgb(40, 40, 40)) } else { Style::new() };
        let mark = if sn.current { "*" } else if sn.unconfirmed { "!" } else { " " };
        let l1 = format!(" {mark} {} · {} · {}{}{}", if sn.app.is_empty() { "приложение" } else { &sn.app }, sn.device, sn.system, if sn.current { "   (это устройство)" } else { "" }, if sn.official { "" } else { "   [неофициальное приложение]" });
        let l2 = format!("     {} · {} · {}", sn.location, sn.last_active, sn.ip);
        ls.push(Line::from(Span::styled(pad_r(&l1, lr.width as usize), if sn.current { bold().fg(acc) } else { Style::new() })).style(bg));
        ls.push(Line::from(Span::styled(pad_r(&l2, lr.width as usize), dim())).style(bg));
    }
    f.rw(Paragraph::new(ls), lr);
    f.rw(Paragraph::new("-".repeat(area.width as usize)).style(dim()), rows[3]);
    if let Some(sn) = a.sessions.get(a.ses_sel) {
        let t = |b: bool| if b { "[x]" } else { "[ ]" };
        let d1 = segs(rows[4].x, rows[4].y, vec![sp(" Для выбранного сеанса:  "), (format!("{} принимать звонки", t(sn.calls)), Style::new().fg(acc), Some(Hit::SesCalls)), sp("   "),
            (format!("{} принимать секретные чаты", t(sn.secret)), Style::new().fg(acc), Some(Hit::SesSecret))], cx);
        f.rw(Paragraph::new(d1), Rect::new(rows[4].x, rows[4].y, rows[4].width, 1));
        if sn.unconfirmed { f.rw(Paragraph::new(" [!] Новый сеанс. Если это не вы, завершите его.").style(Style::new().fg(Color::Yellow)), Rect::new(rows[4].x, rows[4].y + 1, rows[4].width, 1)); }
    }
    let b = segs(rows[5].x, rows[5].y, vec![sp(" "), btn("[ Завершить сеанс ]", acc, Hit::SesKill), sp("  "), btn("[ Завершить все остальные ]", acc, Hit::SesKillAll), sp("  "),
        (format!("[ Автозавершение неактивных: {} дн. ]", a.ses_ttl), Style::new().fg(acc), Some(Hit::SesTtl))], cx);
    f.rw(Paragraph::new(b), rows[5]);
    f.rw(Paragraph::new(" Up/Down выбор  d завершить  A все остальные  c звонки  s секретные чаты  t срок  F5 обновить  Esc назад").style(dim()), rows[7]);
}

// ---------------- Premium и звёзды ----------------
fn draw_shop(f: &mut Frame, a: &App, area: Rect, cx: &mut Ctx) {
    let acc = a.acc();
    let rows = Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1), Constraint::Min(4), Constraint::Length(2), Constraint::Length(1)]).split(area);
    let on = |b: bool| if b { Style::new().fg(Color::Black).bg(acc) } else { Style::new().fg(acc) };
    f.rw(Paragraph::new(segs(rows[0].x, rows[0].y, vec![sp(" "), btn("[<] Назад", acc, Hit::Back), sp("   "),
        ("[ Telegram Premium ]".to_string(), on(a.shop_tab == 0), Some(Hit::ShopTab(0))), sp(" "), ("[ Звёзды ]".to_string(), on(a.shop_tab == 1), Some(Hit::ShopTab(1)))], cx)), rows[0]);
    f.rw(Paragraph::new("-".repeat(area.width as usize)).style(dim()), rows[1]);
    let r = rows[3].inner(&Margin { horizontal: 2, vertical: 0 });
    let mut ls: Vec<Line> = vec![];
    if a.shop_tab == 0 {
        ls.push(Line::from(vec![Span::styled("Статус: ", dim()), Span::styled(if a.me_premium { "Premium активен" } else { "Premium не оформлен" }, bold().fg(if a.me_premium { Color::Yellow } else { Color::Reset }))]));
        ls.push(Line::raw(""));
        for l in wrap(if a.premium_text.is_empty() { "Загрузка описания..." } else { &a.premium_text }, r.width as usize) { ls.push(Line::from(l)); }
        ls.push(Line::raw("")); ls.push(Line::from(Span::styled("Тарифы:", bold())));
        for o in &a.premium_opts { ls.push(Line::from(format!("  - {o}"))); }
    } else {
        ls.push(Line::from(vec![Span::styled("Баланс: ", dim()), Span::styled(match a.stars { Some(b) => format!("{b} звёзд"), None => "загрузка...".into() }, bold().fg(Color::Yellow))]));
        ls.push(Line::raw("")); ls.push(Line::from(Span::styled("Пакеты звёзд:", bold())));
        for o in &a.star_opts { ls.push(Line::from(format!("  - {o}"))); }
    }
    f.rw(Paragraph::new(ls), r);
    f.rw(Paragraph::new(segs(rows[4].x, rows[4].y, vec![sp("  "), btn("[ Оформить / пополнить в @PremiumBot ]", acc, Hit::ShopBuy)], cx)), Rect::new(rows[4].x, rows[4].y, rows[4].width, 1));
    f.rw(Paragraph::new("  Оплата проходит в официальном боте Telegram: в консольном клиенте нет платёжных форм.").style(dim()), Rect::new(rows[4].x, rows[4].y + 1, rows[4].width, 1));
    f.rw(Paragraph::new(" Left/Right вкладка  Enter оплата в браузере  F5 обновить  Esc назад").style(dim()), rows[5]);
}
