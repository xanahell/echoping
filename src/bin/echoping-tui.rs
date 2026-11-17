// echoping: лёгкий неофициальный клиент Telegram для терминала (TDLib + ratatui).
use std::{io, time::Duration};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use echoping::{app::{App, Screen}, mem, testkit, ui};

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
    let mut app = testkit::new_app(tokens);
    let (dw, dh) = dump_size();
    let mut t = Terminal::new(ratatui::backend::TestBackend::new(dw, dh)).unwrap();
    for tok in tokens {
        t.draw(|f| ui::draw(f, &mut app)).unwrap();
        testkit::apply(&mut app, tok);
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
    app.begin();
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
