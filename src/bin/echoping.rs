// echoping: лёгкий неофициальный клиент Telegram в собственном окне (TDLib).
// В Windows запускается без консоли; закрытие окна сворачивает в трей (настраивается).
#![cfg_attr(windows, windows_subsystem = "windows")]

use std::{num::NonZeroU32, sync::Arc, time::{Duration, Instant}};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use echoping::{app::{App, Screen}, mem, render::Renderer, testkit, ui};
use ratatui::{backend::TestBackend, Terminal};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, Ime, MouseButton as WMouse, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    keyboard::{Key, KeyCode as WKey, ModifiersState, NamedKey, PhysicalKey},
    window::{Icon, Window, WindowId},
};

const SCALES: [f32; 5] = [0.75, 1.0, 1.25, 1.5, 2.0];
const BASE_PX: f32 = 17.0;

#[derive(Debug, Clone, Copy)]
#[cfg_attr(not(windows), allow(dead_code))]
enum UserEvent { Wake, TrayShow, TrayQuit }

/// Значок приложения: бирюзовый круг с кольцом «эхо-пинга».
fn icon_rgba(n: u32) -> Vec<u8> {
    let mut v = vec![0u8; (n * n * 4) as usize];
    let c = n as f32 / 2.0;
    for y in 0..n { for x in 0..n {
        let d = (((x as f32 + 0.5 - c).powi(2) + (y as f32 + 0.5 - c).powi(2))).sqrt() / c;
        let i = ((y * n + x) * 4) as usize;
        let (col, a): ([u8; 3], f32) = if d <= 0.30 { ([255, 255, 255], 1.0) }
            else if d >= 0.50 && d <= 0.66 { ([255, 255, 255], 1.0) }
            else if d <= 0.96 { ([36, 160, 184], 1.0) } else { ([0, 0, 0], 0.0) };
        v[i] = col[0]; v[i + 1] = col[1]; v[i + 2] = col[2]; v[i + 3] = (a * 255.0) as u8;
    } }
    v
}

#[cfg(windows)]
mod tray {
    use super::{icon_rgba, UserEvent};
    use tray_icon::{menu::{Menu, MenuEvent, MenuItem}, Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
    use winit::event_loop::EventLoopProxy;

    /// Значок в области уведомлений: щелчок открывает окно, меню содержит «Открыть» и «Выход».
    pub fn start(proxy: EventLoopProxy<UserEvent>) -> Option<TrayIcon> {
        let menu = Menu::new();
        let open = MenuItem::new("Открыть echoping", true, None);
        let quit = MenuItem::new("Выход", true, None);
        menu.append(&open).ok()?;
        menu.append(&quit).ok()?;
        let (oid, qid) = (open.id().clone(), quit.id().clone());
        let p1 = proxy.clone();
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            if e.id == oid { let _ = p1.send_event(UserEvent::TrayShow); } else if e.id == qid { let _ = p1.send_event(UserEvent::TrayQuit); }
        }));
        TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| match e {
            TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } | TrayIconEvent::DoubleClick { button: MouseButton::Left, .. } => { let _ = proxy.send_event(UserEvent::TrayShow); }
            _ => {}
        }));
        TrayIconBuilder::new().with_menu(Box::new(menu)).with_menu_on_left_click(false).with_tooltip("echoping")
            .with_icon(Icon::from_rgba(icon_rgba(32), 32, 32).ok()?).build().ok()
    }
}

/// Текст из буфера обмена (Windows).
#[cfg(windows)]
fn clipboard_text() -> Option<String> {
    extern "system" {
        fn OpenClipboard(h: isize) -> i32;
        fn CloseClipboard() -> i32;
        fn GetClipboardData(fmt: u32) -> isize;
        fn GlobalLock(h: isize) -> *mut u16;
        fn GlobalUnlock(h: isize) -> i32;
    }
    unsafe {
        if OpenClipboard(0) == 0 { return None; }
        let h = GetClipboardData(13); // CF_UNICODETEXT
        let out = if h != 0 {
            let p = GlobalLock(h);
            let s = if p.is_null() { None } else {
                let mut n = 0usize; while *p.add(n) != 0 && n < 1_000_000 { n += 1; }
                Some(String::from_utf16_lossy(std::slice::from_raw_parts(p, n)))
            };
            GlobalUnlock(h);
            s
        } else { None };
        CloseClipboard();
        out
    }
}
#[cfg(not(windows))]
fn clipboard_text() -> Option<String> { None }

struct Gui {
    app: App,
    term: Terminal<TestBackend>,
    rend: Renderer,
    window: Option<Arc<Window>>,
    surface: Option<softbuffer::Surface<Arc<Window>, Arc<Window>>>,
    mods: ModifiersState,
    mouse: (f64, f64),
    left_down: bool,
    scroll_acc: f64,
    dirty: bool,
    cursor_on: bool,
    last_blink: Instant,
    next_tick: Instant,
    last_second: Instant,
    scale_idx: usize,
    visible: bool,
    start_hidden: bool,
    has_tray: bool,
    #[cfg(windows)]
    _tray: Option<tray_icon::TrayIcon>,
    #[cfg_attr(not(windows), allow(dead_code))]
    proxy: EventLoopProxy<UserEvent>,
}

impl Gui {
    fn quit(&mut self) -> ! { self.app.shutdown(); mem::fast_exit(0) }

    fn apply_scale(&mut self) {
        let sf = self.window.as_ref().map(|w| w.scale_factor() as f32).unwrap_or(1.0);
        self.rend.set_px(BASE_PX * sf * SCALES[self.scale_idx]);
        self.dirty = true;
    }
    fn cell_at(&self) -> (u16, u16) {
        ((self.mouse.0.max(0.0) / self.rend.cell_w as f64) as u16, (self.mouse.1.max(0.0) / self.rend.cell_h as f64) as u16)
    }
    fn send_mouse(&mut self, kind: MouseEventKind) {
        let (column, row) = self.cell_at();
        self.app.mouse(MouseEvent { kind, column, row, modifiers: KeyModifiers::NONE });
        self.dirty = true;
    }
    fn zoom(&mut self, d: i32) {
        let n = (self.scale_idx as i32 + d).clamp(0, SCALES.len() as i32 - 1) as usize;
        if n != self.scale_idx { self.scale_idx = n; self.app.st.set("scale", n.to_string()); self.apply_scale(); }
    }
    fn show(&mut self) { if let Some(w) = &self.window { w.set_visible(true); w.focus_window(); self.visible = true; self.dirty = true; } }

    fn on_key(&mut self, ev: &winit::event::KeyEvent) {
        let (ctrl, alt, shift) = (self.mods.control_key(), self.mods.alt_key(), self.mods.shift_key());
        // масштаб интерфейса и вставка
        if ctrl {
            if let Key::Character(s) = &ev.logical_key {
                match s.as_str() { "+" | "=" => { self.zoom(1); return; } "-" | "_" => { self.zoom(-1); return; } "0" => { self.scale_idx = 1; self.app.st.set("scale", "1".into()); self.apply_scale(); return; } _ => {} }
            }
            if let PhysicalKey::Code(WKey::KeyV) = ev.physical_key { if let Some(t) = clipboard_text() { self.app.paste(&t); self.dirty = true; } return; }
        }
        let mut mods = KeyModifiers::NONE;
        if ctrl { mods |= KeyModifiers::CONTROL; }
        if alt { mods |= KeyModifiers::ALT; }
        if shift { mods |= KeyModifiers::SHIFT; }
        let code = match &ev.logical_key {
            Key::Named(n) => match n {
                NamedKey::Enter => KeyCode::Enter, NamedKey::Escape => KeyCode::Esc, NamedKey::Backspace => KeyCode::Backspace, NamedKey::Tab => KeyCode::Tab,
                NamedKey::ArrowUp => KeyCode::Up, NamedKey::ArrowDown => KeyCode::Down, NamedKey::ArrowLeft => KeyCode::Left, NamedKey::ArrowRight => KeyCode::Right,
                NamedKey::PageUp => KeyCode::PageUp, NamedKey::PageDown => KeyCode::PageDown, NamedKey::Home => KeyCode::Home, NamedKey::End => KeyCode::End,
                NamedKey::Delete => KeyCode::Delete, NamedKey::Space => KeyCode::Char(' '),
                NamedKey::F1 => KeyCode::F(1), NamedKey::F2 => KeyCode::F(2), NamedKey::F3 => KeyCode::F(3), NamedKey::F4 => KeyCode::F(4), NamedKey::F5 => KeyCode::F(5),
                NamedKey::F6 => KeyCode::F(6), NamedKey::F7 => KeyCode::F(7), NamedKey::F8 => KeyCode::F(8), NamedKey::F9 => KeyCode::F(9), NamedKey::F10 => KeyCode::F(10),
                NamedKey::F11 => KeyCode::F(11), NamedKey::F12 => KeyCode::F(12),
                _ => return,
            },
            Key::Character(s) => {
                if ctrl || alt {
                    // Горячие клавиши не зависят от раскладки: берём физическую клавишу.
                    let ch = match ev.physical_key {
                        PhysicalKey::Code(c) => match c {
                            WKey::KeyA => 'a', WKey::KeyB => 'b', WKey::KeyC => 'c', WKey::KeyD => 'd', WKey::KeyE => 'e', WKey::KeyF => 'f', WKey::KeyG => 'g', WKey::KeyH => 'h',
                            WKey::KeyI => 'i', WKey::KeyJ => 'j', WKey::KeyK => 'k', WKey::KeyL => 'l', WKey::KeyM => 'm', WKey::KeyN => 'n', WKey::KeyO => 'o', WKey::KeyP => 'p',
                            WKey::KeyQ => 'q', WKey::KeyR => 'r', WKey::KeyS => 's', WKey::KeyT => 't', WKey::KeyU => 'u', WKey::KeyV => 'v', WKey::KeyW => 'w', WKey::KeyX => 'x',
                            WKey::KeyY => 'y', WKey::KeyZ => 'z', _ => s.chars().next().unwrap_or(' ').to_ascii_lowercase(),
                        },
                        _ => s.chars().next().unwrap_or(' ').to_ascii_lowercase(),
                    };
                    self.app.key(KeyEvent::new(KeyCode::Char(ch), mods));
                    self.dirty = true;
                    return;
                }
                let t = ev.text.as_ref().map(|t| t.to_string()).unwrap_or_else(|| s.to_string());
                for c in t.chars().filter(|c| !c.is_control()) { self.app.key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)); }
                self.dirty = true;
                return;
            }
            _ => return,
        };
        self.app.key(KeyEvent::new(code, mods));
        self.dirty = true;
    }

    fn redraw(&mut self) {
        let Some(win) = self.window.clone() else { return };
        let size = win.inner_size();
        let (w, h) = (size.width, size.height);
        if w == 0 || h == 0 || !self.visible { return; }
        let (cols, rows) = self.rend.grid(w, h);
        if self.term.backend().buffer().area.width != cols || self.term.backend().buffer().area.height != rows { self.term.backend_mut().resize(cols, rows); }
        self.app.view_px = (w * 8 / 10, h * 7 / 10);
        let _ = self.term.draw(|f| ui::draw(f, &mut self.app));
        if let Some(surf) = self.surface.as_mut() {
            if let (Some(nw), Some(nh)) = (NonZeroU32::new(w), NonZeroU32::new(h)) {
                let _ = surf.resize(nw, nh);
                if let Ok(mut buf) = surf.buffer_mut() {
                    self.rend.draw(&mut buf, w, h, self.term.backend().buffer(), &self.app, self.app.cursor, self.cursor_on);
                    let _ = buf.present();
                }
            }
        }
    }
}

impl ApplicationHandler<UserEvent> for Gui {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() { return; }
        let icon = Icon::from_rgba(icon_rgba(32), 32, 32).ok();
        let attrs = Window::default_attributes().with_title("echoping").with_inner_size(LogicalSize::new(1100.0, 720.0))
            .with_min_inner_size(LogicalSize::new(360.0, 240.0)).with_window_icon(icon).with_visible(!self.start_hidden);
        let Ok(win) = el.create_window(attrs) else { el.exit(); return };
        let win = Arc::new(win);
        win.set_ime_allowed(true);
        if let Ok(ctx) = softbuffer::Context::new(win.clone()) { self.surface = softbuffer::Surface::new(&ctx, win.clone()).ok(); }
        self.visible = !self.start_hidden;
        self.window = Some(win);
        self.apply_scale();
        #[cfg(windows)]
        { self._tray = tray::start(self.proxy.clone()); self.has_tray = self._tray.is_some(); }
        self.app.begin();
        self.dirty = true;
    }

    fn user_event(&mut self, _el: &ActiveEventLoop, ev: UserEvent) {
        match ev {
            UserEvent::Wake => { if self.app.pump() > 0 { self.dirty = true; } }
            UserEvent::TrayShow => self.show(),
            UserEvent::TrayQuit => self.quit(),
        }
    }

    fn window_event(&mut self, _el: &ActiveEventLoop, _id: WindowId, ev: WindowEvent) {
        match ev {
            WindowEvent::CloseRequested => {
                if self.has_tray && self.app.st.on("close_tray") { if let Some(w) = &self.window { w.set_visible(false); } self.visible = false; }
                else { self.quit(); }
            }
            WindowEvent::Resized(_) => self.dirty = true,
            WindowEvent::ScaleFactorChanged { .. } => self.apply_scale(),
            WindowEvent::ModifiersChanged(m) => self.mods = m.state(),
            WindowEvent::KeyboardInput { event, .. } => { if event.state == ElementState::Pressed { self.on_key(&event); } }
            WindowEvent::DroppedFile(path) => { self.app.drop_file(&path.to_string_lossy()); self.dirty = true; }
            WindowEvent::Ime(Ime::Commit(t)) => { self.app.paste(&t); self.dirty = true; }
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse = (position.x, position.y);
                if self.left_down { self.send_mouse(MouseEventKind::Drag(MouseButton::Left)); }
            }
            WindowEvent::MouseInput { state, button, .. } => match (state, button) {
                (ElementState::Pressed, WMouse::Left) => { self.left_down = true; self.send_mouse(MouseEventKind::Down(MouseButton::Left)); }
                (ElementState::Released, WMouse::Left) => { self.left_down = false; self.send_mouse(MouseEventKind::Up(MouseButton::Left)); }
                (ElementState::Pressed, WMouse::Right) => self.send_mouse(MouseEventKind::Down(MouseButton::Right)),
                _ => {}
            },
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta { MouseScrollDelta::LineDelta(_, y) => y as f64, MouseScrollDelta::PixelDelta(p) => { self.scroll_acc += p.y; let l = (self.scroll_acc / self.rend.cell_h as f64).trunc(); self.scroll_acc -= l * self.rend.cell_h as f64; l } };
                if self.mods.control_key() { if lines > 0.0 { self.zoom(1); } else if lines < 0.0 { self.zoom(-1); } return; }
                for _ in 0..lines.abs().ceil() as usize { self.send_mouse(if lines > 0.0 { MouseEventKind::ScrollUp } else { MouseEventKind::ScrollDown }); }
            }
            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        let now = Instant::now();
        if now >= self.next_tick {
            self.next_tick = now + Duration::from_millis(250);
            if self.app.pump() > 0 { self.dirty = true; }
            self.app.tick();
            self.app.housekeeping();
            if self.app.animating() { self.dirty = true; }
            let idx = self.app.st.idx("scale").min(SCALES.len() - 1);
            if idx != self.scale_idx { self.scale_idx = idx; self.apply_scale(); }
            if now.duration_since(self.last_blink) >= Duration::from_millis(530) {
                self.last_blink = now; self.cursor_on = !self.cursor_on;
                if self.app.cursor.is_some() { self.dirty = true; }
            }
            if now.duration_since(self.last_second) >= Duration::from_secs(1) { self.last_second = now; if self.visible { self.dirty = true; } }
        }
        if self.app.quit { self.quit(); }
        if self.dirty && self.visible { self.dirty = false; if let Some(w) = &self.window { w.request_redraw(); } }
        el.set_control_flow(ControlFlow::WaitUntil(self.next_tick));
    }
}

/// Режим проверки без окна: собирает интерфейс, рисует его тем же отрисовщиком и сохраняет PNG.
fn render_png(path: &str, tokens: &[String]) {
    let (w, h) = (1100u32, 720u32);
    let mut app = testkit::new_app(tokens);
    app.gui = true;
    app.view_px = (w * 8 / 10, h * 7 / 10);
    let scale = tokens.iter().find_map(|t| t.strip_prefix("scale:")).and_then(|v| v.parse::<f32>().ok()).unwrap_or(1.0);
    let mut rend = Renderer::new(BASE_PX * scale);
    let (cols, rows) = rend.grid(w, h);
    let mut term = Terminal::new(TestBackend::new(cols, rows)).unwrap();
    for tok in tokens { let _ = term.draw(|f| ui::draw(f, &mut app)); testkit::apply(&mut app, tok); }
    let _ = term.draw(|f| ui::draw(f, &mut app));
    let mut buf = vec![0u32; (w * h) as usize];
    rend.draw(&mut buf, w, h, term.backend().buffer(), &app, app.cursor, true);
    let mut rgb = Vec::with_capacity((w * h * 3) as usize);
    for p in &buf { rgb.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, *p as u8]); }
    image::save_buffer(path, &rgb, w, h, image::ColorType::Rgb8).expect("png");
    mem::fast_exit(0);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(i) = args.iter().position(|a| a == "--render-png") { render_png(&args[i + 1], &args[i + 2..]); }
    let el = EventLoop::<UserEvent>::with_user_event().build().expect("event loop");
    el.set_control_flow(ControlFlow::Wait);
    let proxy = el.create_proxy();
    let demo = args.iter().any(|a| a == "--demo");
    let mut app = App::new(demo);
    app.gui = true;
    if demo { app.screen = Screen::Main; }
    let p2 = proxy.clone();
    app.wake = Some(Arc::new(move || { let _ = p2.send_event(UserEvent::Wake); }));
    let scale_idx = app.st.idx("scale").min(SCALES.len() - 1);
    let start_hidden = app.st.on("start_min") && cfg!(windows);
    let rend = Renderer::new(BASE_PX * SCALES[scale_idx]);
    let term = Terminal::new(TestBackend::new(80, 24)).unwrap();
    let mut gui = Gui { app, term, rend, window: None, surface: None, mods: ModifiersState::empty(), mouse: (0.0, 0.0), left_down: false, scroll_acc: 0.0,
        dirty: true, cursor_on: true, last_blink: Instant::now(), next_tick: Instant::now(), last_second: Instant::now(), scale_idx, visible: true, start_hidden,
        has_tray: false, #[cfg(windows)] _tray: None, proxy };
    let _ = el.run_app(&mut gui);
}
