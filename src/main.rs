// echoping: Telegram-клиент для терминала (TDLib + ratatui).
mod app; mod avatar; mod data; mod mem; mod player; mod settings; mod td; mod ui;
use std::{io, time::Duration};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use app::{App, Screen};
use td::{Auth, ChatInfo, Ev};

fn dump(app: &mut App, w: u16, h: u16) {
    let mut t = Terminal::new(ratatui::backend::TestBackend::new(w, h)).unwrap();
    t.draw(|f| ui::draw(f, app)).unwrap();
    let b = t.backend().buffer().clone();
    for y in 0..h {
        let mut l = String::new();
        for x in 0..w { l.push_str(b.get(x, y).symbol()); }
        println!("{}", l.trim_end());
    }
    if std::env::var("DUMP_COLORS").is_ok() {
        for y in 0..h { for x in 0..w { let c = b.get(x, y); if c.symbol() == "▀" { eprintln!("{y} {x} {:?} {:?}", c.fg, c.bg); } } }
    }
}

/// Режим проверки: --dump click:x,y key:Enter text:abc ... печатает кадр в текст.
fn run_dump(tokens: &[String]) {
    let mut app = App::new(!tokens.iter().any(|t| t == "real"));
    let (dw, dh) = dump_size();
    let mut t = Terminal::new(ratatui::backend::TestBackend::new(dw, dh)).unwrap();
    for tok in tokens {
        t.draw(|f| ui::draw(f, &mut app)).unwrap();
        if let Some(v) = tok.strip_prefix("click:").or(tok.strip_prefix("rclick:")) {
            let (x, y) = v.split_once(',').unwrap();
            let kind = if tok.starts_with('r') { MouseEventKind::Down(MouseButton::Right) } else { MouseEventKind::Down(MouseButton::Left) };
            app.mouse(MouseEvent { kind, column: x.parse().unwrap(), row: y.parse().unwrap(), modifiers: KeyModifiers::NONE });
        } else if let Some(v) = tok.strip_prefix("scroll:") {
            let (x, y) = v.split_once(',').unwrap();
            app.mouse(MouseEvent { kind: MouseEventKind::ScrollUp, column: x.parse().unwrap(), row: y.parse().unwrap(), modifiers: KeyModifiers::NONE });
        } else if let Some(v) = tok.strip_prefix("text:") {
            for c in v.chars() { app.key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)); }
        } else if let Some(v) = tok.strip_prefix("key:") {
            let (code, m) = match v {
                "Enter" => (KeyCode::Enter, KeyModifiers::NONE), "Esc" => (KeyCode::Esc, KeyModifiers::NONE), "Tab" => (KeyCode::Tab, KeyModifiers::NONE),
                "Up" => (KeyCode::Up, KeyModifiers::NONE), "Down" => (KeyCode::Down, KeyModifiers::NONE), "Left" => (KeyCode::Left, KeyModifiers::NONE),
                "Right" => (KeyCode::Right, KeyModifiers::NONE), "PgUp" => (KeyCode::PageUp, KeyModifiers::NONE), "F1" => (KeyCode::F(1), KeyModifiers::NONE),
                "F2" => (KeyCode::F(2), KeyModifiers::NONE), "F3" => (KeyCode::F(3), KeyModifiers::NONE),
                "ctrl-g" => (KeyCode::Char('g'), KeyModifiers::CONTROL), "alt-p" => (KeyCode::Char('p'), KeyModifiers::ALT), "ctrl-f" => (KeyCode::Char('f'), KeyModifiers::CONTROL), "F9" => (KeyCode::F(9), KeyModifiers::NONE), "ctrl-o" => (KeyCode::Char('o'), KeyModifiers::CONTROL),
                _ => (KeyCode::Null, KeyModifiers::NONE),
            };
            app.key(KeyEvent::new(code, m));
        } else if let Some(v) = tok.strip_prefix("ev:") {
            // Проверочные события, имитирующие ответы TDLib (реальной сети в песочнице нет).
            let mk = |id: i64, t: &str, k: char, un: i32, last: &str, out: bool| ChatInfo { id, title: t.into(), kind: k, status: "в сети".into(), unread: un,
                muted: false, order: 1000 - id, pinned: id == 1, last: Some({ let mut m = data::msg(out, t, "14 ноября 2026 г.", "12:30", last); m.id = id * 10; m }), user_id: 0, read_out: 0, arch_order: 0, photo: None, saved: false };
            match v {
                "qr" => { app.login.step = app::LStep::Wait; app.on_event(Ev::Auth(Auth::Qr("tg://login?token=AbCdEfGhIjKlMnOpQrStUvWxYz0123456789_-AbCdEfGhIj".into()))) }
                "code" => app.on_event(Ev::Auth(Auth::WaitCode { phone: "+31612345678".into(), via: "в приложение Telegram".into() })),
                "pass" => app.on_event(Ev::Auth(Auth::WaitPassword { hint: "любимый город".into() })),
                "err" => app.on_event(Ev::Fail("Неверный пароль".into())),
                "ready" => app.on_event(Ev::Auth(Auth::Ready)),
                "saved" => { let mut c = mk(9, "Избранное", 'p', 0, "заметка", true); c.saved = true; c.order = 0; app.on_event(Ev::ChatNew(c)); }
                "arch" => { let mut c = mk(8, "Старая группа", 'g', 0, "давно", false); c.arch_order = 500; c.order = 0; app.on_event(Ev::ChatNew(c)); }
                "call" => app.on_event(Ev::Call { info: td::CallInfo { id: 1, user: "Мама".into(), incoming: true, video: false, state: "вызов...".into() }, ended: false }),
                "many" => { for i in 1..=30 { app.on_event(Ev::ChatNew(mk(i, &format!("Chat number {i}"), 'p', 0, "hello", false))); } }
                "chats" => {
                    app.on_event(Ev::ChatNew(mk(1, "Мама \u{1F496}", 'p', 2, "Привет! \u{1F44B} Как дела? Позвони вечером \u{1F60A}", false)));
                    app.on_event(Ev::ChatNew(mk(2, "Работа 日本語", 'g', 0, "Созвон в 15:00", true)));
                    app.on_event(Ev::ChatNew(mk(3, "Новости", 'c', 7, "Вышла новая версия", false)));
                }
                _ => {}
            }
        } else if let Some(v) = tok.strip_prefix("wait:") {
            let ms: u64 = v.parse().unwrap_or(500);
            let t0 = std::time::Instant::now();
            while t0.elapsed().as_millis() < ms as u128 { app.pump(); app.tick(); std::thread::sleep(Duration::from_millis(50)); }
        } else if tok == "nologin" { app.screen = Screen::Main; }
    }
    dump(&mut app, dw, dh);
    if std::env::var("DUMP_MEM").is_ok() {
        let a = mem::rss_mb(); mem::trim(); let b = mem::rss_mb();
        eprintln!("RSS {a:.1} МБ -> после освобождения {b:.1} МБ");
    }
    if std::env::var("DUMP_HITS").is_ok() { for (r, h) in &app.hits { eprintln!("{} {} {} {} {:?}", r.x, r.y, r.width, r.height, h); } }
    { use std::io::Write; let _ = io::stdout().flush(); mem::fast_exit(0); }
}

fn dump_size() -> (u16, u16) {
    std::env::var("DUMP_SIZE").ok().and_then(|v| v.split_once('x').map(|(a, b)| (a.parse().unwrap_or(120), b.parse().unwrap_or(32)))).unwrap_or((120, 32))
}

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(i) = args.iter().position(|a| a == "--dump") { run_dump(&args[i + 1..]); return Ok(()); }
    let demo = args.iter().any(|a| a == "--demo" || a == "--no-login");
    let mut app = App::new(demo);
    app.mouse_dbg = args.iter().any(|a| a == "--mouse-debug");
    if args.iter().any(|a| a == "--no-login") { app.screen = Screen::Main; }
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |p| { let _ = disable_raw_mode(); let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture); default_hook(p); }));
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let mut mouse_on = app.st.on("mouse");
    if mouse_on { execute!(io::stdout(), EnableMouseCapture)?; }
    let mut term = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut dirty = true;
    let mut last_draw = std::time::Instant::now();
    while !app.quit {
        if dirty || last_draw.elapsed() >= Duration::from_millis(if app.animating() { 250 } else { 1000 }) {
            term.draw(|f| ui::draw(f, &mut app))?;
            dirty = false; last_draw = std::time::Instant::now();
        }
        // Спим дольше, когда нечего делать: меньше пробуждений и нагрузки на процессор.
        if event::poll(Duration::from_millis(if app.be.is_some() { 80 } else { 250 }))? {
            match event::read()? {
                Event::Key(k) => app.key(k),
                Event::Mouse(m) => app.mouse(m),
                _ => {}
            }
            dirty = true;
        }
        if app.pump() > 0 { dirty = true; }
        app.tick();
        app.housekeeping();
        if app.mouse_changed {
            app.mouse_changed = false;
            let want = app.st.on("mouse");
            if want != mouse_on { mouse_on = want; if want { execute!(io::stdout(), EnableMouseCapture)?; } else { execute!(io::stdout(), DisableMouseCapture)?; } }
        }
    }
    app.shutdown();
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
    { use std::io::Write; let _ = io::stdout().flush(); }
    mem::fast_exit(0)
}
