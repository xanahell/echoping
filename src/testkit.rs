//! Вспомогательные команды для автоматической проверки интерфейса (кадры без реального окна и терминала).
//! Используются режимами --dump (терминал) и --render-png (окно).
use std::time::Duration;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use crate::{app::{self, App, Screen}, data, td::{self, Auth, ChatInfo, Ev}};

/// Создаёт приложение для проверки: демо по умолчанию, с токеном `real` запускается TDLib.
pub fn new_app(tokens: &[String]) -> App {
    let mut app = App::new(!tokens.iter().any(|t| t == "real"));
    if tokens.iter().any(|t| t == "real") { app.begin(); }
    app
}

/// Применяет одну проверочную команду: click:x,y rclick:x,y scroll:x,y scrolld:x,y text:abc key:Enter ev:имя wait:мс nologin
pub fn apply(app: &mut App, tok: &str) {
    if let Some(v) = tok.strip_prefix("click:").or(tok.strip_prefix("rclick:")) {
        let (x, y) = v.split_once(',').unwrap();
        let kind = if tok.starts_with('r') { MouseEventKind::Down(MouseButton::Right) } else { MouseEventKind::Down(MouseButton::Left) };
        app.mouse(MouseEvent { kind, column: x.parse().unwrap(), row: y.parse().unwrap(), modifiers: KeyModifiers::NONE });
    } else if let Some(v) = tok.strip_prefix("scroll:") {
        let (x, y) = v.split_once(',').unwrap();
        app.mouse(MouseEvent { kind: MouseEventKind::ScrollUp, column: x.parse().unwrap(), row: y.parse().unwrap(), modifiers: KeyModifiers::NONE });
    } else if let Some(v) = tok.strip_prefix("scrolld:") {
        let (x, y) = v.split_once(',').unwrap();
        app.mouse(MouseEvent { kind: MouseEventKind::ScrollDown, column: x.parse().unwrap(), row: y.parse().unwrap(), modifiers: KeyModifiers::NONE });
    } else if let Some(v) = tok.strip_prefix("text:") {
        for c in v.chars() { app.key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)); }
    } else if tok == "addacct" { app.add_account();
    } else if let Some(v) = tok.strip_prefix("key:") {
        let (code, m) = if let Some(c) = v.strip_prefix("ctrl-").filter(|c| c.chars().count() == 1) { (KeyCode::Char(c.chars().next().unwrap()), KeyModifiers::CONTROL) } else { match v {
            "Enter" => (KeyCode::Enter, KeyModifiers::NONE), "Esc" => (KeyCode::Esc, KeyModifiers::NONE), "Tab" => (KeyCode::Tab, KeyModifiers::NONE),
            "Up" => (KeyCode::Up, KeyModifiers::NONE), "Down" => (KeyCode::Down, KeyModifiers::NONE), "Left" => (KeyCode::Left, KeyModifiers::NONE),
            "Right" => (KeyCode::Right, KeyModifiers::NONE), "PgUp" => (KeyCode::PageUp, KeyModifiers::NONE), "F1" => (KeyCode::F(1), KeyModifiers::NONE),
            "F2" => (KeyCode::F(2), KeyModifiers::NONE), "F3" => (KeyCode::F(3), KeyModifiers::NONE),
            "ctrl-e" => (KeyCode::Char('e'), KeyModifiers::CONTROL), "ctrl-g" => (KeyCode::Char('g'), KeyModifiers::CONTROL), "alt-p" => (KeyCode::Char('p'), KeyModifiers::ALT), "ctrl-f" => (KeyCode::Char('f'), KeyModifiers::CONTROL), "F9" => (KeyCode::F(9), KeyModifiers::NONE), "ctrl-o" => (KeyCode::Char('o'), KeyModifiers::CONTROL),
            _ => (KeyCode::Null, KeyModifiers::NONE),
        } };
        app.key(KeyEvent::new(code, m));
    } else if let Some(v) = tok.strip_prefix("ev:") {
        // Проверочные события, имитирующие ответы TDLib (реальной сети в песочнице нет).
        let mk = |id: i64, t: &str, k: char, un: i32, last: &str, out: bool| ChatInfo { id, title: t.into(), kind: k, status: "в сети".into(), unread: un,
            muted: false, order: 1000 - id, pinned: id == 1, last: Some({ let mut m = data::msg(out, t, "14 ноября 2026 г.", "12:30", last); m.id = id * 10; m }), user_id: 0, read_out: 0, arch_order: 0, photo: None, saved: false, read_only: false, sg_id: 0, bg_id: 0 };
        match v {
            "qr" => { app.login.step = app::LStep::Wait; app.on_event(Ev::Auth(Auth::Qr("tg://login?token=AbCdEfGhIjKlMnOpQrStUvWxYz0123456789_-AbCdEfGhIj".into()))) }
            "code" => app.on_event(Ev::Auth(Auth::WaitCode { phone: "+31612345678".into(), via: "в приложение Telegram".into() })),
            "pass" => app.on_event(Ev::Auth(Auth::WaitPassword { hint: "любимый город".into() })),
            "err" => app.on_event(Ev::Fail("Неверный пароль".into())),
            "ready" => app.on_event(Ev::Auth(Auth::Ready)),
            "saved" => { let mut c = mk(9, "Избранное", 'p', 0, "заметка", true); c.saved = true; c.order = 0; app.on_event(Ev::ChatNew(c)); }
            "arch" => { let mut c = mk(8, "Старая группа", 'g', 0, "давно", false); c.arch_order = 500; c.order = 0; app.on_event(Ev::ChatNew(c)); }
            "call" => app.on_event(Ev::Call { info: td::CallInfo { id: 1, user: "Мама".into(), incoming: true, video: false, state: "вызов...".into() }, ended: false }),
            "ro" => { let mut c = mk(7, "Канал новостей", 'c', 0, "Только чтение", false); c.read_only = true; app.on_event(Ev::ChatNew(c)); }
            "tabs6" => { app.tabs = (0..6).collect(); }
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
