// Состояние и логика приложения: клавиатура, мышь, вход, настройки, плеер.
// Методы api_* — точки, куда подключается TDLib. Режим невидимки работает
// именно здесь: запросы «прочитано» / «прослушано» / «печатает» просто не отправляются.
use std::{collections::{HashMap, HashSet}, path::Path, time::{Duration, Instant}};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{layout::Rect, style::Color};
use crate::{avatar::{self, Photo}, data::*, player::Player, settings::{config_dir, Kind as SKind, Settings, SECTIONS}, td::{Auth, Backend, CallInfo, Cmd, Ev, SessionInfo}};

pub const FOLDERS: [&str; 7] = ["Все", "Личные", "Группы", "Каналы", "Боты", "★", "Архив"];
pub const LEFT_W: u16 = 40;
pub const SES_TTL: [i32; 5] = [7, 30, 90, 180, 365];
pub const MENU: [&str; 8] = ["Ответить", "Копировать", "Переслать", "Изменить", "Открыть / Играть", "Сохранить файл", "Закрепить", "Удалить"];

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Hit {
    Search, Folder(usize), Chat(usize), ListArea, Tab(usize), TabClose(usize),
    HdrSearch, HdrCall, HdrInfo, HdrMenu, Ghost, Attach, Emoji, SendBtn, MicBtn, InputBox,
    Msg(usize), MsgArea, MenuItem(usize), BannerX,
    MiniPrev, MiniPlay, MiniNext, MiniSeek, MiniClose, MiniOpen,
    OpenSettings, OpenPlayer, Back,
    SetSection(usize), SetItem(usize), SetArea,
    LoginPhone, LoginScan, LoginNext, LoginBack, LoginEye, LoginApi,
    ChatMenuItem(usize), CallDecline, SesRow(usize), SesKill, SesKillAll, SesCalls, SesSecret, SesTtl, ShopTab(usize), ShopBuy,
    PlTrack(usize), PlArea, PlPrev, PlPlay, PlNext, PlShuffle, PlRepeat, PlVolDown, PlVolUp, PlSeek,
}

#[derive(PartialEq, Clone, Copy)] pub enum Focus { Search, List, Messages, Input }
#[derive(PartialEq, Clone, Copy)] pub enum Screen { Login, Main }
#[derive(PartialEq, Clone, Copy)] pub enum Page { Chats, Settings, Player, Sessions, Shop }
#[derive(PartialEq, Clone, Copy)] pub enum LStep { Api, Wait, Qr, Phone, Code, Pass }

pub struct Login {
    pub step: LStep, pub phone: String, pub code: String, pub pass: String,
    pub show: bool, pub err: String, pub qr: Vec<Vec<bool>>,
    pub api_id: String, pub api_hash: String, pub field: usize,
    pub status: String, pub hint: String, pub code_hint: String, pub qr_requested: bool,
}

pub struct App {
    pub screen: Screen, pub page: Page, pub login: Login, pub st: Settings,
    pub chats: Vec<Chat>, pub folder: usize, pub sel: usize, pub focus: Focus,
    pub search: String, pub input: String, pub msg_sel: Option<usize>,
    pub menu: Option<usize>, pub menu_pos: Option<(u16, u16)>,
    pub info: bool, pub help: bool, pub reply: Option<usize>, pub edit: Option<usize>,
    pub tabs: Vec<usize>, pub notice: String,
    pub scroll_up: usize, pub max_scroll: usize, pub list_off: usize, pub list_cap: usize,
    pub hits: Vec<(Rect, Hit)>,
    pub set_sec: usize, pub set_item: usize, pub set_col: usize, pub set_edit: Option<String>,
    pub player: Player,
    pub voice: Option<(usize, usize, Instant)>,
    pub photos: HashMap<String, Option<Photo>>,
    pub quit: bool, pub mouse_changed: bool,
    pub demo: bool, pub be: Option<Backend>, sel_id: Option<i64>,
    pub me_id: i64, pub me_premium: bool, pub me_name: String, pub stars: Option<i64>,
    pub avatars: HashMap<i64, Photo>, av_req: HashSet<i64>,
    pub attach: Option<String>, pub chat_menu: Option<(usize, (u16, u16), usize)>,
    pub sessions: Vec<SessionInfo>, pub ses_ttl: i32, pub ses_sel: usize,
    pub shop_tab: usize, pub premium_text: String, pub premium_opts: Vec<String>, pub star_opts: Vec<String>,
    pub call: Option<CallInfo>, pub mouse_dbg: bool, pub last_input: Instant, last_trim: Instant, last_load: Instant, last_ram: Instant,
    last_click: Option<(usize, Instant)>,
}

fn parse_dur(s: &str) -> f32 {
    let mut p = s.split(':');
    let m: f32 = p.next().and_then(|x| x.parse().ok()).unwrap_or(0.0);
    let sec: f32 = p.next().and_then(|x| x.parse().ok()).unwrap_or(0.0);
    m * 60.0 + sec
}

fn launch(target: &str) -> bool {
    #[cfg(windows)]
    { return std::process::Command::new("cmd").args(["/C", "start", "", target]).spawn().is_ok(); }
    #[cfg(not(windows))]
    { return std::process::Command::new(if cfg!(target_os = "macos") { "open" } else { "xdg-open" }).arg(target).spawn().is_ok(); }
}
fn open_external(p: &Path) -> bool { p.exists() && launch(&p.to_string_lossy()) }
const PREMIUM_URL: &str = "https://t.me/PremiumBot";

fn demo_link() -> String {
    let mut x = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(7) | 1;
    let abc = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let tok: String = (0..22).map(|_| { x ^= x << 13; x ^= x >> 7; x ^= x << 17; abc[(x % 62) as usize] as char }).collect();
    format!("tg://login?token={tok}") // ДЕМО-токен, не рабочий
}

fn make_qr(link: &str) -> Vec<Vec<bool>> {
    let Ok(code) = qrcode::QrCode::with_error_correction_level(link.as_bytes(), qrcode::EcLevel::L) else { return vec![] };
    let w = code.width();
    code.to_colors().chunks(w).map(|r| r.iter().map(|c| *c == qrcode::Color::Dark).collect()).collect()
}

impl App {
    pub fn new(demo: bool) -> Self {
        let st = Settings::load();
        let mut player = Player::new();
        player.volume = (st.idx("pl_vol") * 25) as u8;
        player.shuffle = st.on("pl_shuffle");
        player.repeat = st.idx("pl_repeat") as u8;
        let env_or = |k: &str, key: &str| std::env::var(k).ok().filter(|v| !v.is_empty()).unwrap_or_else(|| st.get(key));
        let (api_id, api_hash) = (env_or("ECHOPING_API_ID", "api_id"), env_or("ECHOPING_API_HASH", "api_hash"));
        let mut a = App {
            screen: Screen::Login, page: Page::Chats,
            login: Login { step: if demo { LStep::Qr } else { LStep::Api }, phone: String::new(), code: String::new(), pass: String::new(), show: false,
                err: String::new(), qr: if demo { make_qr(&demo_link()) } else { vec![] }, api_id, api_hash, field: 0,
                status: String::new(), hint: String::new(), code_hint: String::new(), qr_requested: false },
            st, chats: if demo { mock_chats() } else { vec![] }, folder: 0, sel: if demo { 1 } else { 0 }, focus: Focus::List, search: String::new(), input: String::new(),
            msg_sel: None, menu: None, menu_pos: None, info: false, help: false, reply: None, edit: None, tabs: vec![],
            notice: "F1 справка".into(), scroll_up: 0, max_scroll: 0, list_off: 0, list_cap: 10, hits: vec![],
            set_sec: 0, set_item: 0, set_col: 0, set_edit: None, player, voice: None, photos: HashMap::new(),
            quit: false, mouse_changed: false, demo, be: None, sel_id: None, last_click: None,
            me_id: 0, me_premium: false, me_name: String::new(), stars: None, avatars: HashMap::new(), av_req: HashSet::new(),
            attach: None, chat_menu: None, sessions: vec![], ses_ttl: 180, ses_sel: 0, shop_tab: 0, premium_text: String::new(),
            premium_opts: vec![], star_opts: vec![], call: None, mouse_dbg: false, last_input: Instant::now(), last_trim: Instant::now(), last_load: Instant::now(), last_ram: Instant::now(),
        };
        if !demo && !a.login.api_id.is_empty() && !a.login.api_hash.is_empty() { a.start_backend(); }
        a.on_select();
        a
    }
    /// Запуск TDLib (только когда известны api_id и api_hash).
    fn start_backend(&mut self) {
        let Ok(id) = self.login.api_id.trim().parse::<i32>() else { self.login.err = "api_id должен быть числом".into(); self.login.step = LStep::Api; return };
        let hash = self.login.api_hash.trim().to_string();
        if hash.len() != 32 { self.login.err = "api_hash состоит из 32 символов".into(); self.login.step = LStep::Api; return; }
        self.st.set("api_id", id.to_string());
        self.st.set("api_hash", hash.clone());
        self.be = Some(Backend::start(id, hash, config_dir().join("tdlib"), self.st.on("lowmem")));
        self.login.step = LStep::Wait; self.login.err.clear(); self.login.qr_requested = false;
        self.login.status = "Запуск TDLib...".into();
    }
    fn cmd(&self, c: Cmd) { if let Some(b) = &self.be { b.send(c); } }
    fn sync_online(&self) { self.cmd(Cmd::SetOnline(!(self.ghost() && self.st.on("ghost_offline")))); }
    pub fn shutdown(&mut self) {
        if let Some(b) = &self.be {
            b.send(Cmd::Close);
            let t = Instant::now();
            while t.elapsed() < Duration::from_secs(3) {
                if let Ok(Ev::Auth(Auth::Closed)) = b.rx.recv_timeout(Duration::from_millis(100)) { break; }
            }
        }
    }
    pub fn acc(&self) -> Color {
        match self.st.idx("accent") { 0 => Color::Cyan, 1 => Color::LightBlue, 2 => Color::Green, 3 => Color::Magenta, _ => Color::Yellow }
    }
    pub fn ghost(&self) -> bool { self.st.on("ghost") }
    fn g_read(&self) -> bool { self.ghost() && self.st.on("ghost_read") }
    fn g_listen(&self) -> bool { self.ghost() && self.st.on("ghost_listen") }
    #[allow(dead_code)]
    pub fn g_typing(&self) -> bool { self.ghost() && self.st.on("ghost_typing") }

    pub fn ensure_photos(&mut self) {
        if !self.st.on("avatars") { return; }
        for c in &self.chats {
            if !self.photos.contains_key(&c.name) { self.photos.insert(c.name.clone(), avatar::load_photo(&c.name)); }
        }
    }

    // ---------- чаты ----------
    pub fn visible(&self) -> Vec<usize> {
        let q = self.search.to_lowercase();
        let mut v: Vec<usize> = (0..self.chats.len()).filter(|&i| {
            let c = &self.chats[i];
            let in_folder = match self.folder {
                1 => c.order > 0 && c.kind == 'p' && !c.saved, 2 => c.order > 0 && c.kind == 'g', 3 => c.order > 0 && c.kind == 'c', 4 => c.order > 0 && c.kind == 'b',
                5 => c.saved || (c.order > 0 && c.pinned), // избранное: сохранённые сообщения и закреплённые чаты
                6 => c.arch_order > 0,
                _ => c.order > 0,
            };
            in_folder && c.name.to_lowercase().contains(&q)
        }).collect();
        if self.folder == 6 { v.sort_by_key(|&i| std::cmp::Reverse(self.chats[i].arch_order)); }
        else { v.sort_by_key(|&i| (!self.chats[i].saved, std::cmp::Reverse(self.chats[i].order))); }
        v
    }
    pub fn cur(&self) -> Option<usize> { self.visible().get(self.sel).copied() }
    fn on_select(&mut self) {
        self.msg_sel = None; self.reply = None; self.edit = None; self.input.clear(); self.scroll_up = 0;
        self.sel_id = self.cur().map(|c| self.chats[c].id);
        if let Some(c) = self.cur() {
            if !self.demo && !self.chats[c].loaded { self.chats[c].loaded = true; let id = self.chats[c].id; self.cmd(Cmd::History(id)); }
            self.api_view(c);
        }
    }
    /// После изменения порядка чатов возвращаем выделение на тот же чат.
    fn resync(&mut self) {
        if let Some(id) = self.sel_id {
            if let Some(p) = self.visible().iter().position(|&i| self.chats[i].id == id) { self.sel = p; }
        }
    }
    fn ensure_visible(&mut self) {
        let cap = self.list_cap.max(1);
        if self.sel < self.list_off { self.list_off = self.sel; }
        else if self.sel >= self.list_off + cap { self.list_off = self.sel + 1 - cap; }
    }
    fn move_sel(&mut self, d: i32) {
        let n = self.visible().len() as i32;
        if n == 0 { return; }
        self.sel = (self.sel as i32 + d).clamp(0, n - 1) as usize;
        self.ensure_visible();
        self.on_select();
    }
    fn select_chat_id(&mut self, id: usize) {
        self.folder = 0; self.search.clear();
        if let Some(p) = self.visible().iter().position(|&x| x == id) { self.sel = p; self.ensure_visible(); self.on_select(); }
    }
    /// Чат по-настоящему «открыт» только после выбора: тогда грузится история и (если не невидимка) отмечается прочитанным.
    fn ensure_open(&mut self) {
        let cur_id = self.cur().map(|c| self.chats[c].id);
        if cur_id.is_some() && cur_id != self.sel_id { self.on_select(); }
    }
    fn open_current_tab(&mut self) {
        self.ensure_open();
        if let Some(c) = self.cur() { if !self.tabs.contains(&c) { self.tabs.push(c); } }
        self.focus = Focus::Input;
    }

    // ---------- точки подключения TDLib ----------
    /// TDLib: viewMessages («прочитано»). В режиме невидимки запрос НЕ отправляется.
    fn api_view(&mut self, c: usize) {
        if self.g_read() {
            if self.chats[c].unread > 0 { self.notice = "[~] невидимка: чат открыт, «прочитано» не отправлено".into(); }
            return;
        }
        self.chats[c].unread = 0;
        if !self.demo {
            let ids: Vec<i64> = self.chats[c].msgs.iter().rev().filter(|m| !m.out).take(50).map(|m| m.id).collect();
            if !ids.is_empty() { let chat = self.chats[c].id; self.cmd(Cmd::View { chat, ids }); }
        }
    }
    /// TDLib: openMessageContent («прослушано / просмотрено»). В невидимке НЕ отправляется.
    fn api_open(&mut self, c: usize, i: usize) {
        if self.g_listen() { self.notice = "[~] невидимка: «прослушано» не отправлено".into(); return; }
        self.chats[c].msgs[i].listened = true;
        if !self.demo { let (chat, msg) = (self.chats[c].id, self.chats[c].msgs[i].id); self.cmd(Cmd::OpenContent { chat, msg }); }
    }

    fn send(&mut self) {
        let Some(c) = self.cur() else { return };
        let text = self.input.trim().to_string();
        if text.is_empty() { return; }
        if !self.demo {
            let chat = self.chats[c].id;
            if let Some(i) = self.edit.take() { let msg = self.chats[c].msgs[i].id; self.cmd(Cmd::Edit { chat, msg, text }); }
            else { let reply = self.reply.take().map(|i| self.chats[c].msgs[i].id); self.cmd(Cmd::Send { chat, text, reply }); }
            self.input.clear(); self.msg_sel = None; self.scroll_up = 0;
            return;
        }
        if let Some(i) = self.edit.take() {
            let m = &mut self.chats[c].msgs[i]; m.text = text; m.edited = true;
        } else {
            let quote = self.reply.take().map(|i| {
                let m = &self.chats[c].msgs[i];
                if m.text.is_empty() { "[вложение]".to_string() } else { m.text.replace('\n', " ") }
            });
            let now = chrono::Local::now().format("%H:%M").to_string();
            let mut m = msg(true, "Вы", D2, &now, &text);
            m.read = false; m.reply = quote;
            self.chats[c].msgs.push(m);
            // Отправка сообщения обычно отмечает чат прочитанным даже в невидимке.
            if self.g_read() && self.chats[c].unread > 0 { self.notice = "[~] ответ в чат мог отметить его прочитанным".into(); }
            self.chats[c].unread = 0;
        }
        self.input.clear(); self.msg_sel = None; self.scroll_up = 0;
    }

    fn toggle_voice(&mut self, c: usize, i: usize) {
        if matches!(self.voice, Some((cc, ii, _)) if cc == c && ii == i) { self.voice = None; return; }
        self.voice = Some((c, i, Instant::now()));
        self.api_open(c, i);
        self.notice = "(демо) воспроизведение голосового: Opus-декодер появится вместе с TDLib".into();
    }
    pub fn open_msg(&mut self, c: usize, i: usize) {
        let kind = self.chats[c].msgs[i].kind.clone();
        if !self.demo {
            if let Some(file) = self.chats[c].msgs[i].file {
                self.api_open(c, i);
                let name = match &kind { Kind::File(n, _) | Kind::Image(n, _) => n.clone(), Kind::Voice(_) => format!("голосовое_{file}"), _ => format!("видео_{file}.mp4") };
                self.notice = "Скачиваю...".into();
                self.cmd(Cmd::Download { file, name, open: true });
            } else { self.notice = "Здесь нечего открывать".into(); }
            return;
        }
        match kind {
            Kind::Voice(_) => self.toggle_voice(c, i),
            Kind::Video(_) => {
                self.api_open(c, i);
                if !self.g_listen() { self.notice = "(демо) видеосообщение откроется во внешнем плеере системы".into(); }
            }
            Kind::File(n, _) | Kind::Image(n, _) => {
                self.api_open(c, i);
                let p = self.st.dl_dir().join(&n);
                self.notice = if open_external(&p) { format!("Открыто: {n}") } else { format!("Файл {n} ещё не скачан (демо)") };
            }
            _ => self.notice = "Здесь нечего открывать".into(),
        }
    }
    fn menu_do(&mut self, item: usize) {
        let Some(c) = self.cur() else { return };
        let Some(i) = self.msg_sel else { return };
        match item {
            0 => { self.reply = Some(i); self.focus = Focus::Input; }
            1 => self.notice = "(демо) текст скопирован".into(),
            3 => {
                if self.chats[c].msgs[i].out && matches!(self.chats[c].msgs[i].kind, Kind::Text) {
                    self.input = self.chats[c].msgs[i].text.clone(); self.edit = Some(i); self.focus = Focus::Input;
                } else { self.notice = "Менять можно только свои текстовые сообщения".into(); }
            }
            4 => self.open_msg(c, i),
            5 => {
                if self.demo {
                    match &self.chats[c].msgs[i].kind {
                        Kind::File(n, _) | Kind::Image(n, _) => self.notice = format!("(демо) сохранено в {}", self.st.dl_dir().join(n).display()),
                        _ => self.notice = "В сообщении нет файла".into(),
                    }
                } else if let (Some(file), Kind::File(n, _) | Kind::Image(n, _)) = (self.chats[c].msgs[i].file, self.chats[c].msgs[i].kind.clone()) {
                    self.notice = "Скачиваю...".into();
                    self.cmd(Cmd::Download { file, name: n, open: false });
                } else { self.notice = "В сообщении нет файла".into(); }
            }
            7 => {
                if !self.demo { let (chat, id) = (self.chats[c].id, self.chats[c].msgs[i].id); self.cmd(Cmd::Delete { chat, ids: vec![id] }); }
                self.chats[c].msgs.remove(i); self.msg_sel = None; self.focus = Focus::Input;
            }
            _ => self.notice = "Пока не реализовано".into(),
        }
    }

    pub fn tick(&mut self) {
        if let Some((c, i, t)) = self.voice {
            let d = if let Kind::Voice(s) = &self.chats[c].msgs[i].kind { parse_dur(s) } else { 0.0 };
            if t.elapsed().as_secs_f32() >= d { self.voice = None; }
        }
        self.player.tick(self.st.on("pl_autonext"));
    }

    // ---------- вход ----------
    fn login_enter(&mut self) {
        self.login.err.clear();
        match self.login.step {
            LStep::Api => {
                if self.login.field == 0 { self.login.field = 1; } else { self.start_backend(); }
            }
            LStep::Wait => {}
            LStep::Qr => {
                if self.demo { self.login.step = LStep::Pass; }
                else { self.login.err = "Отсканируйте QR-код в Telegram на телефоне".into(); }
            }
            LStep::Phone => {
                let digits: String = self.login.phone.chars().filter(|c| c.is_ascii_digit()).collect();
                if digits.len() < 7 { self.login.err = "Введите номер полностью, с кодом страны".into(); }
                else if self.demo { self.login.step = LStep::Code; }
                else { self.login.status = "Запрашиваю код...".into(); self.cmd(Cmd::Phone(format!("+{digits}"))); }
            }
            LStep::Code => {
                if self.login.code.len() != 5 { self.login.err = "Код состоит из 5 цифр".into(); }
                else if self.demo { self.login.step = LStep::Pass; }
                else { let c = self.login.code.clone(); self.login.status = "Проверяю код...".into(); self.cmd(Cmd::Code(c)); }
            }
            LStep::Pass => {
                if self.login.pass.is_empty() { self.login.err = "Введите облачный пароль".into(); return; }
                if self.demo { self.login.pass.clear(); self.login.show = false; self.screen = Screen::Main; self.page = Page::Chats; self.notice = "Вход выполнен (демо)".into(); }
                else { let p = std::mem::take(&mut self.login.pass); self.login.status = "Проверяю пароль...".into(); self.cmd(Cmd::Password(p)); }
            }
        }
    }
    fn login_back(&mut self) {
        self.login.err.clear();
        match self.login.step {
            LStep::Phone => {
                self.login.step = LStep::Qr;
                if !self.demo { self.login.qr_requested = true; self.login.status = "Запрашиваю QR-код...".into(); self.cmd(Cmd::RequestQr); }
            }
            LStep::Code => { self.login.step = LStep::Phone; self.login.code.clear(); if !self.demo { self.cmd(Cmd::Restart); } }
            LStep::Pass => {
                self.login.pass.clear(); self.login.step = LStep::Qr;
                if !self.demo { self.login.qr_requested = false; self.login.qr.clear(); self.cmd(Cmd::Restart); } else { self.login.qr = make_qr(&demo_link()); }
            }
            _ => {}
        }
    }
    /// Переход «по номеру телефона» из экрана QR. В TDLib после запроса QR вернуться к вводу номера можно только перезапуском клиента.
    fn login_to_phone(&mut self) {
        self.login.step = LStep::Phone; self.login.err.clear();
        if !self.demo { self.login.qr_requested = false; self.cmd(Cmd::Restart); }
    }
    fn login_key(&mut self, k: KeyEvent) {
        let step = self.login.step;
        match k.code {
            KeyCode::Enter => { self.login_enter(); return; }
            KeyCode::Esc => { if matches!(step, LStep::Phone | LStep::Code | LStep::Pass) { self.login_back(); } return; }
            _ => {}
        }
        let l = &mut self.login;
        match (step, k.code) {
            (LStep::Api, KeyCode::Tab) => l.field = 1 - l.field,
            (LStep::Api, KeyCode::Char(c)) if !c.is_whitespace() => { if l.field == 0 { if c.is_ascii_digit() { l.api_id.push(c) } } else if l.api_hash.len() < 32 { l.api_hash.push(c) } }
            (LStep::Api, KeyCode::Backspace) => { if l.field == 0 { l.api_id.pop(); } else { l.api_hash.pop(); } }
            (LStep::Qr | LStep::Wait, KeyCode::Char('k' | 'K')) => { l.step = LStep::Api; l.field = 0; }
            (LStep::Qr, KeyCode::Char('p' | 'P')) => self.login_to_phone(),
            (LStep::Phone, KeyCode::Char(c)) if c.is_ascii_digit() || c == '+' || c == ' ' => l.phone.push(c),
            (LStep::Phone, KeyCode::Backspace) => { l.phone.pop(); }
            (LStep::Code, KeyCode::Char(c)) if c.is_ascii_digit() && l.code.len() < 5 => l.code.push(c),
            (LStep::Code, KeyCode::Backspace) => { l.code.pop(); }
            (LStep::Pass, KeyCode::Char(c)) => l.pass.push(c),
            (LStep::Pass, KeyCode::Backspace) => { l.pass.pop(); }
            (LStep::Pass, KeyCode::Tab) => l.show = !l.show,
            _ => {}
        }
    }

    // ---------- события от TDLib ----------
    pub fn pump(&mut self) -> usize {
        let mut n = 0;
        loop {
            let ev = match self.be.as_ref().and_then(|b| b.rx.try_recv().ok()) { Some(e) => e, None => break };
            self.on_event(ev);
            n += 1; if n > 400 { break; } // не задерживать отрисовку при лавине обновлений
        }
        if self.be.is_some() && self.screen == Screen::Main && self.page == Page::Chats { self.request_avatars(); self.maybe_load_more(); }
        n
    }
    /// Аватарки скачиваются только для чатов, которые сейчас видны в списке (экономия памяти и трафика).
    fn request_avatars(&mut self) {
        if !self.st.on("avatars") { return; }
        let vis = self.visible();
        let (a, b) = (self.list_off, (self.list_off + self.list_cap + 1).min(vis.len()));
        let mut want: Vec<(i64, i32)> = vis.get(a..b).unwrap_or(&[]).iter().filter_map(|&i| self.chats[i].photo.map(|f| (self.chats[i].id, f))).collect();
        if let Some(c) = self.cur() { if let Some(f) = self.chats[c].photo { want.push((self.chats[c].id, f)); } }
        for (key, file) in want { if self.av_req.insert(key) { self.cmd(Cmd::Avatar { key, file }); } }
    }
    /// Список чатов подгружается порциями, когда пользователь доходит до его конца.
    fn maybe_load_more(&mut self) {
        if self.last_load.elapsed() < Duration::from_millis(1500) { return; }
        let n = self.visible().len();
        if self.list_off + self.list_cap + 3 >= n {
            self.last_load = Instant::now();
            let archive = self.folder == 6;
            self.cmd(Cmd::LoadChats { archive });
        }
    }
    /// Нужна ли частая перерисовка (воспроизведение, анимации).
    pub fn animating(&self) -> bool { self.player.playing || self.voice.is_some() }
    /// Расход памяти и освобождение: вызывается из главного цикла.
    pub fn housekeeping(&mut self) {
        if self.last_ram.elapsed() >= Duration::from_secs(2) {
            self.last_ram = Instant::now();
            self.st.set_rt("ram_now", format!("{:.1} МБ", crate::mem::rss_mb()));
        }
        // В простое возвращаем системе неиспользуемые страницы памяти.
        if self.st.on("trim") && self.last_input.elapsed() > Duration::from_secs(3) && self.last_trim.elapsed() >= Duration::from_secs(10) {
            crate::mem::trim(); self.last_trim = Instant::now();
        }
    }
    fn chat_idx(&self, id: i64) -> Option<usize> { self.chats.iter().position(|c| c.id == id) }
    fn on_auth(&mut self, a: Auth) {
        match a {
            Auth::WaitPhone => {
                if self.screen == Screen::Login && (self.login.step == LStep::Wait || (self.login.step == LStep::Qr && !self.login.qr_requested)) {
                    self.login.step = LStep::Qr; self.login.qr_requested = true; self.login.status = "Запрашиваю QR-код...".into();
                    self.cmd(Cmd::RequestQr);
                }
            }
            Auth::Qr(link) => {
                if matches!(self.login.step, LStep::Qr | LStep::Wait) {
                    self.login.qr = make_qr(&link); self.login.step = LStep::Qr; self.login.status = "Ожидаю сканирования...".into(); self.login.err.clear();
                }
            }
            Auth::WaitCode { phone, via } => { self.login.step = LStep::Code; self.login.code.clear(); self.login.code_hint = format!("Код отправлен {via} ({phone})"); self.login.err.clear(); }
            Auth::WaitPassword { hint } => { self.login.step = LStep::Pass; self.login.pass.clear(); self.login.hint = hint; self.login.err.clear(); }
            Auth::Ready => {
                self.screen = Screen::Main; self.page = Page::Chats; self.login.qr.clear();
                self.notice = "Вход выполнен".into();
                self.cmd(Cmd::LoadChats { archive: false }); self.cmd(Cmd::EnsureSaved); self.sync_online();
            }
            Auth::Unsupported(t) => { self.login.err = t; }
            Auth::Closed => {}
        }
    }
    fn put_msg(c: &mut Chat, mut m: Msg) {
        if m.out { m.read = m.id <= c.read_out; }
        if let Some(p) = c.msgs.iter().position(|x| x.id == m.id) { c.msgs[p] = m; }
        else { let pos = c.msgs.iter().position(|x| x.id > m.id).unwrap_or(c.msgs.len()); c.msgs.insert(pos, m); }
        if c.msgs.len() > 150 { c.msgs.remove(0); } // ограничение памяти
    }
    pub fn on_event(&mut self, ev: Ev) {
        match ev {
            Ev::Auth(a) => self.on_auth(a),
            Ev::Fail(t) => { if self.screen == Screen::Login { self.login.err = t; self.login.status.clear(); } else { self.notice = t; } }
            Ev::ApiInvalid => { self.login.err = "Telegram отклонил api_id / api_hash. Проверьте ключи.".into(); self.login.step = LStep::Api; self.login.field = 0; self.screen = Screen::Login; }
            Ev::ChatNew(ci) => {
                let mut c = Chat { id: ci.id, order: ci.order, user_id: ci.user_id, read_out: ci.read_out, name: ci.title, kind: ci.kind, status: ci.status,
                    unread: ci.unread.max(0) as u32, pinned: ci.pinned, muted: ci.muted, arch_order: ci.arch_order, photo: ci.photo, saved: ci.saved, ..Default::default() };
                if let Some(l) = ci.last { Self::put_msg(&mut c, l); }
                match self.chat_idx(ci.id) { Some(i) => { let old = std::mem::take(&mut self.chats[i].msgs); let loaded = self.chats[i].loaded; self.chats[i] = c; self.chats[i].loaded = loaded; if !old.is_empty() { self.chats[i].msgs = old; } } None => self.chats.push(c) }
                self.resync();
            }
            Ev::ChatLast { id, last, order, pinned, arch_order } => if let Some(i) = self.chat_idx(id) {
                self.chats[i].order = order; self.chats[i].pinned = pinned; self.chats[i].arch_order = arch_order;
                if let Some(l) = last { Self::put_msg(&mut self.chats[i], l); }
                self.resync();
            },
            Ev::ChatPos { id, archive, order, pinned } => if let Some(i) = self.chat_idx(id) {
                if archive { self.chats[i].arch_order = order; } else { self.chats[i].order = order; self.chats[i].pinned = pinned; }
                self.resync();
            },
            Ev::ChatUnread { id, count } => if let Some(i) = self.chat_idx(id) { self.chats[i].unread = count.max(0) as u32; },
            Ev::ChatTitle { id, title } => if let Some(i) = self.chat_idx(id) { self.chats[i].name = title; },
            Ev::ChatMute { id, muted } => if let Some(i) = self.chat_idx(id) { self.chats[i].muted = muted; },
            Ev::ChatReadOut { id, msg } => if let Some(i) = self.chat_idx(id) {
                self.chats[i].read_out = msg;
                for m in self.chats[i].msgs.iter_mut().filter(|m| m.out && m.id <= msg) { m.read = true; }
            },
            Ev::UserStatus { user_id, status } => {
                for c in self.chats.iter_mut().filter(|c| c.user_id == user_id && c.kind == 'p') { c.status = status.clone(); }
            }
            Ev::NewMsg(chat, m) => if let Some(i) = self.chat_idx(chat) {
                let (id, incoming) = (m.id, !m.out);
                Self::put_msg(&mut self.chats[i], m);
                if incoming && self.cur() == Some(i) && !self.g_read() {
                    self.cmd(Cmd::View { chat, ids: vec![id] });
                }
            },
            Ev::MsgContent { chat, id, text, kind, file, listened } => if let Some(i) = self.chat_idx(chat) {
                if let Some(m) = self.chats[i].msgs.iter_mut().find(|m| m.id == id) { m.text = text; m.kind = kind; m.file = file; m.listened = m.out || listened; m.edited = true; }
            },
            Ev::Replaced { chat, old, msg } => if let Some(i) = self.chat_idx(chat) {
                self.chats[i].msgs.retain(|m| m.id != old);
                Self::put_msg(&mut self.chats[i], msg);
            },
            Ev::Deleted { chat, ids } => if let Some(i) = self.chat_idx(chat) {
                self.chats[i].msgs.retain(|m| !ids.contains(&m.id));
                self.msg_sel = None;
            },
            Ev::History { chat, msgs } => if let Some(i) = self.chat_idx(chat) {
                for m in msgs { Self::put_msg(&mut self.chats[i], m); }
                if self.cur() == Some(i) && !self.g_read() && self.chats[i].unread > 0 { self.api_view(i); }
            },
            Ev::Me { id, name, username, phone, premium, photo } => {
                self.me_id = id; self.me_premium = premium; self.me_name = name.clone();
                self.st.set_rt("name", name);
                self.st.set_rt("username", if username.is_empty() { "не задано".into() } else { format!("@{username}") });
                self.st.set_rt("phone", if phone.is_empty() { "скрыт".into() } else { format!("+{phone}") });
                let ps = if premium { "активен" } else { "нет" }.to_string();
                self.st.set_rt("premium_state", ps.clone()); self.st.set_rt("premium_state2", ps);
                if let Some(f) = photo { if self.av_req.insert(-1) { self.cmd(Cmd::Avatar { key: -1, file: f }); } }
            }
            Ev::Avatar { key, photo } => { self.avatars.insert(key, photo); }
            Ev::OpenChat(id) => {
                if let Some(i) = self.chat_idx(id) {
                    self.folder = 5; self.search.clear(); self.list_off = 0;
                    if let Some(p) = self.visible().iter().position(|&x| x == i) { self.sel = p; self.ensure_visible(); self.on_select(); self.open_current_tab(); }
                }
            }
            Ev::Sessions { list, ttl } => { self.sessions = list; self.ses_ttl = ttl; self.ses_sel = self.ses_sel.min(self.sessions.len().saturating_sub(1)); }
            Ev::Premium { text, options } => { self.premium_text = text; self.premium_opts = options; }
            Ev::Stars { balance, options } => { self.stars = Some(balance); self.star_opts = options; self.st.set_rt("stars_state", format!("{balance}")); self.st.set_rt("stars_state2", format!("{balance}")); }
            Ev::StarBalance(b) => { self.stars = Some(b); self.st.set_rt("stars_state", format!("{b}")); self.st.set_rt("stars_state2", format!("{b}")); }
            Ev::Call { info, ended } => {
                if ended { if self.call.as_ref().map(|c| c.id) == Some(info.id) { self.call = None; } self.notice = format!("Звонок с {} завершён", info.user); }
                else { if info.incoming && self.call.is_none() { print!("\x07"); } self.call = Some(info); }
            }
            Ev::Downloaded { path, name, open } => {
                let p = std::path::PathBuf::from(&path);
                if open {
                    self.notice = if open_external(&p) { format!("Открыто: {name}") } else { format!("Не удалось открыть {name}") };
                } else {
                    let dir = self.st.dl_dir();
                    let _ = std::fs::create_dir_all(&dir);
                    let dst = dir.join(&name);
                    self.notice = match std::fs::copy(&p, &dst) { Ok(_) => format!("Сохранено: {}", dst.display()), Err(e) => format!("Не удалось сохранить: {e}") };
                }
            }
        }
    }

    // ---------- клавиатура ----------
    pub fn key(&mut self, k: KeyEvent) {
        if k.kind != KeyEventKind::Press { return; }
        self.last_input = Instant::now();
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && k.code == KeyCode::Char('q') { self.quit = true; return; }
        match self.screen {
            Screen::Login => self.login_key(k),
            Screen::Main => {
                if self.help { self.help = false; return; }
                if self.chat_menu_key(k) { return; }
                if self.menu_key(k) { return; }
                match k.code {
                    KeyCode::Char('g') if ctrl => { let v = !self.ghost(); self.st.set("ghost", if v { "1" } else { "0" }.into()); self.sync_online();
                        self.notice = if v { "[~] режим невидимки включён".into() } else { "Режим невидимки выключен".into() }; return; }
                    KeyCode::Char('o') if ctrl => { self.page = Page::Settings; return; }
                    KeyCode::F(3) => { self.open_player(); return; }
                    KeyCode::F(6) => { self.player.prev(); return; }
                    KeyCode::F(7) => { self.player.toggle(); return; }
                    KeyCode::F(8) => { self.player.next(); return; }
                    KeyCode::F(9) => { self.decline_call(); return; }
                    KeyCode::Char('f') if ctrl && self.page == Page::Chats => { self.start_attach(); return; }
                    _ => {}
                }
                match self.page { Page::Chats => self.chats_key(k, ctrl), Page::Settings => self.settings_key(k), Page::Player => self.player_key(k), Page::Sessions => self.sessions_key(k), Page::Shop => self.shop_key(k) }
            }
        }
    }
    fn menu_key(&mut self, k: KeyEvent) -> bool {
        let Some(m) = self.menu else { return false };
        match k.code {
            KeyCode::Up => self.menu = Some(m.saturating_sub(1)),
            KeyCode::Down => self.menu = Some((m + 1).min(MENU.len() - 1)),
            KeyCode::Enter => { self.menu = None; self.menu_do(m); }
            KeyCode::Esc => self.menu = None,
            _ => {}
        }
        true
    }
    fn open_player(&mut self) {
        if !self.player.scanned { let d = self.st.music_dir(); self.player.scan(&d); }
        self.page = Page::Player;
    }

    fn chats_key(&mut self, k: KeyEvent, ctrl: bool) {
        let alt = k.modifiers.contains(KeyModifiers::ALT);
        match k.code {
            KeyCode::Char('k') if ctrl => { self.focus = Focus::Search; return; }
            KeyCode::Char('w') if ctrl => { if let Some(c) = self.cur() { self.tabs.retain(|&t| t != c); } return; }
            KeyCode::Up if ctrl => { self.move_sel(-1); return; }
            KeyCode::Down if ctrl => { self.move_sel(1); return; }
            KeyCode::F(1) => { self.help = true; return; }
            KeyCode::F(2) => { self.info = !self.info; return; }
            KeyCode::Char(d) if alt && d.is_ascii_digit() => {
                let n = d.to_digit(10).unwrap() as usize;
                if n >= 1 && n <= self.tabs.len() { let id = self.tabs[n - 1]; self.select_chat_id(id); }
                return;
            }
            KeyCode::Tab => { if self.focus == Focus::List { self.ensure_open(); self.focus = Focus::Input; } else { self.focus = Focus::List; } return; }
            _ => {}
        }
        let Some(c) = self.cur() else {
            if self.focus == Focus::Search {
                match k.code { KeyCode::Char(ch) => self.search.push(ch), KeyCode::Backspace => { self.search.pop(); }
                    KeyCode::Esc => { self.search.clear(); self.focus = Focus::List; } _ => {} }
            }
            return;
        };
        let n_msgs = self.chats[c].msgs.len();
        match self.focus {
            Focus::Search => match k.code {
                KeyCode::Char(ch) => { self.search.push(ch); self.sel = 0; self.list_off = 0; self.on_select(); }
                KeyCode::Backspace => { self.search.pop(); self.sel = 0; self.list_off = 0; self.on_select(); }
                KeyCode::Esc => { self.search.clear(); self.focus = Focus::List; }
                KeyCode::Enter | KeyCode::Down => self.focus = Focus::List,
                _ => {}
            },
            Focus::List => match k.code {
                KeyCode::Up => self.move_sel(-1),
                KeyCode::Down => self.move_sel(1),
                KeyCode::Left => { self.folder = self.folder.saturating_sub(1); self.sel = 0; self.list_off = 0; self.on_select(); }
                KeyCode::Right => { self.folder = (self.folder + 1).min(FOLDERS.len() - 1); self.sel = 0; self.list_off = 0; self.on_select(); }
                KeyCode::Enter => self.open_current_tab(),
                KeyCode::Char('p') if alt => self.chat_action(0),
                KeyCode::Char('a') if alt => self.chat_action(1),
                _ => {}
            },
            Focus::Input if self.attach.is_some() => match k.code {
                KeyCode::Char(ch) if !ctrl => self.attach.as_mut().unwrap().push(ch),
                KeyCode::Backspace => { self.attach.as_mut().unwrap().pop(); }
                KeyCode::Enter => self.send_file(),
                KeyCode::Esc => self.attach = None,
                _ => {}
            },
            Focus::Input => match k.code {
                KeyCode::Char(ch) if !ctrl => { self.input.push(ch); /* TDLib: sendChatAction(typing), если !g_typing() */ }
                KeyCode::Backspace => { self.input.pop(); }
                KeyCode::Enter => self.send(),
                KeyCode::Esc => {
                    if self.reply.is_some() || self.edit.is_some() { self.reply = None; self.edit = None; self.input.clear(); }
                    else { self.focus = Focus::List; }
                }
                KeyCode::Up if self.input.is_empty() => {
                    if let Some(i) = self.chats[c].msgs.iter().rposition(|m| m.out && matches!(m.kind, Kind::Text)) {
                        self.input = self.chats[c].msgs[i].text.clone(); self.edit = Some(i);
                    }
                }
                KeyCode::PageUp if n_msgs > 0 => { self.focus = Focus::Messages; self.msg_sel = Some(n_msgs - 1); }
                _ => {}
            },
            Focus::Messages => {
                let i = self.msg_sel.unwrap_or(n_msgs.saturating_sub(1));
                match k.code {
                    KeyCode::Up | KeyCode::PageUp => self.msg_sel = Some(i.saturating_sub(1)),
                    KeyCode::Down | KeyCode::PageDown => {
                        if i + 1 >= n_msgs { self.msg_sel = None; self.focus = Focus::Input; } else { self.msg_sel = Some(i + 1); }
                    }
                    KeyCode::Enter => { self.menu = Some(0); self.menu_pos = None; }
                    KeyCode::Char('r') => self.menu_do(0),
                    KeyCode::Char('e') => self.menu_do(3),
                    KeyCode::Char('o') | KeyCode::Char(' ') => self.menu_do(4),
                    KeyCode::Char('s') => self.menu_do(5),
                    KeyCode::Char('d') => self.menu_do(7),
                    KeyCode::Esc => { self.msg_sel = None; self.focus = Focus::Input; }
                    _ => {}
                }
            }
        }
    }

    // ---------- чаты: закрепление, архив, файлы, звонки ----------
    pub fn chat_menu_items(&self, ci: usize) -> Vec<String> {
        let c = &self.chats[ci];
        vec![if c.pinned { "Открепить".into() } else { "Закрепить в избранном".into() }, if c.arch_order > 0 { "Вернуть из архива".into() } else { "Отправить в архив".into() }]
    }
    fn chat_menu_key(&mut self, k: KeyEvent) -> bool {
        let Some((ci, pos, m)) = self.chat_menu else { return false };
        match k.code {
            KeyCode::Up => self.chat_menu = Some((ci, pos, m.saturating_sub(1))),
            KeyCode::Down => self.chat_menu = Some((ci, pos, (m + 1).min(1))),
            KeyCode::Enter => { self.chat_menu = None; self.chat_action_for(ci, m); }
            KeyCode::Esc => self.chat_menu = None,
            _ => {}
        }
        true
    }
    fn chat_action(&mut self, item: usize) { if let Some(ci) = self.cur() { self.chat_action_for(ci, item); } }
    fn chat_action_for(&mut self, ci: usize, item: usize) {
        let (id, pinned, archived) = (self.chats[ci].id, self.chats[ci].pinned, self.chats[ci].arch_order > 0);
        if item == 0 {
            if self.demo { let c = &mut self.chats[ci]; c.pinned = !pinned; c.order += if pinned { -1_000_000 } else { 1_000_000 }; }
            else { self.cmd(Cmd::Pin { chat: id, pin: !pinned }); }
            self.notice = if pinned { "Чат откреплён".into() } else { "Чат закреплён (виден во вкладке ★)".into() };
        } else {
            if self.demo {
                let c = &mut self.chats[ci];
                if archived { c.order = c.arch_order; c.arch_order = 0; } else { c.arch_order = c.order.max(1); c.order = 0; }
            } else { self.cmd(Cmd::Archive { chat: id, archive: !archived }); }
            self.notice = if archived { "Чат возвращён из архива".into() } else { "Чат отправлен в архив (вкладка Архив)".into() };
        }
        self.resync();
    }
    fn start_attach(&mut self) {
        if self.cur().is_none() { return; }
        self.ensure_open(); self.focus = Focus::Input; self.attach = Some(String::new());
        self.notice = "Введите путь к файлу или перетащите файл в окно, затем Enter".into();
    }
    fn send_file(&mut self) {
        let Some(raw) = self.attach.take() else { return };
        let path = raw.trim().trim_matches('"').trim_matches('\'').to_string();
        let Some(c) = self.cur() else { return };
        if path.is_empty() { return; }
        let p = Path::new(&path);
        if !p.is_file() { self.notice = format!("Файл не найден: {path}"); return; }
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let size = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
        if size > 2_000_000_000 { self.notice = "Файл больше 2 ГБ: Telegram не принимает".into(); return; }
        if self.demo {
            let now = chrono::Local::now().format("%H:%M").to_string();
            let mut m = msg(true, "Вы", D2, &now, ""); m.kind = Kind::File(name.clone(), format!("{:.1} МБ", size as f64 / 1_048_576.0));
            self.chats[c].msgs.push(m);
            self.notice = format!("(демо) файл {name} отправлен");
        } else {
            let chat = self.chats[c].id;
            let reply = self.reply.take().map(|i| self.chats[c].msgs[i].id);
            self.cmd(Cmd::SendFile { chat, path, reply });
            self.notice = format!("Отправляю {name}...");
        }
        self.scroll_up = 0; self.msg_sel = None;
    }
    fn decline_call(&mut self) {
        if let Some(c) = &self.call {
            if c.incoming { let id = c.id; self.cmd(Cmd::DeclineCall(id)); self.notice = "Звонок отклонён".into(); }
            self.call = None;
        }
    }

    // ---------- активные сеансы ----------
    fn open_sessions(&mut self) {
        self.page = Page::Sessions; self.ses_sel = 0;
        if self.demo {
            let mk = |id: i64, cur: bool, app: &str, dev: &str, loc: &str, t: &str| SessionInfo { id, current: cur, unconfirmed: false, calls: true, secret: !cur, official: true,
                app: app.into(), device: dev.into(), system: "Windows 11".into(), location: loc.into(), ip: "192.0.2.1".into(), last_active: t.into() };
            self.sessions = vec![mk(1, true, "echoping 0.1", "PC", "Netherlands", "сейчас"), mk(2, false, "Telegram Android 11.x", "Pixel 8", "Netherlands", "14.11.2026 09:12"), mk(3, false, "Telegram Web", "Chrome", "Germany", "10.11.2026 20:01")];
        } else { self.cmd(Cmd::Sessions); }
    }
    fn session_act(&mut self, what: u8) {
        let Some(sn) = self.sessions.get(self.ses_sel) else { return };
        let (id, cur, calls, secret) = (sn.id, sn.current, sn.calls, sn.secret);
        match what {
            0 => { // завершить выбранный
                if cur { self.notice = "Текущий сеанс завершается через «Выйти из аккаунта»".into(); return; }
                if self.demo { self.sessions.retain(|x| x.id != id); } else { self.cmd(Cmd::KillSession(id)); }
                self.notice = "Сеанс завершён".into();
            }
            1 => { if self.demo { self.sessions.retain(|x| x.current); } else { self.cmd(Cmd::KillOthers); } self.notice = "Все остальные сеансы завершены".into(); }
            2 => { if self.demo { self.sessions[self.ses_sel].calls = !calls; } else { self.cmd(Cmd::SessionCalls { id, on: !calls }); } }
            3 => { if self.demo { self.sessions[self.ses_sel].secret = !secret; } else { self.cmd(Cmd::SessionSecret { id, on: !secret }); } }
            _ => { // срок автозавершения неактивных
                let i = SES_TTL.iter().position(|&d| d >= self.ses_ttl).unwrap_or(0);
                let d = SES_TTL[(i + 1) % SES_TTL.len()];
                self.ses_ttl = d;
                if !self.demo { self.cmd(Cmd::SessionTtl(d)); }
            }
        }
        self.ses_sel = self.ses_sel.min(self.sessions.len().saturating_sub(1));
    }
    fn sessions_key(&mut self, k: KeyEvent) {
        let n = self.sessions.len();
        match k.code {
            KeyCode::Esc => self.page = Page::Settings,
            KeyCode::Up => self.ses_sel = self.ses_sel.saturating_sub(1),
            KeyCode::Down => if n > 0 { self.ses_sel = (self.ses_sel + 1).min(n - 1) },
            KeyCode::Char('d') | KeyCode::Delete => self.session_act(0),
            KeyCode::Char('A') => self.session_act(1),
            KeyCode::Char('c') => self.session_act(2),
            KeyCode::Char('s') => self.session_act(3),
            KeyCode::Char('t') => self.session_act(4),
            KeyCode::F(5) => if !self.demo { self.cmd(Cmd::Sessions) },
            _ => {}
        }
    }

    // ---------- Premium и звёзды ----------
    fn open_shop(&mut self, tab: usize) {
        self.page = Page::Shop; self.shop_tab = tab;
        if self.demo {
            self.premium_text = "(демо) Premium: больше лимитов, быстрая загрузка, голос в текст.".into();
            self.premium_opts = vec!["1 мес. за 4.99 USD".into(), "12 мес. за 47.99 USD (скидка 20%)".into()];
            self.star_opts = vec!["50 звёзд за 0.99 USD".into(), "500 звёзд за 9.99 USD".into()];
            if self.stars.is_none() { self.stars = Some(0); }
        } else { self.cmd(if tab == 0 { Cmd::Premium } else { Cmd::Stars }); }
    }
    fn buy(&mut self) {
        // Платёж выполняется в официальном боте Telegram: в консольном клиенте нет платёжных форм.
        self.notice = if launch(PREMIUM_URL) { "Открываю @PremiumBot: оплата проходит там".into() } else { format!("Откройте {PREMIUM_URL}") };
    }
    fn shop_key(&mut self, k: KeyEvent) {
        match k.code {
            KeyCode::Esc => self.page = Page::Settings,
            KeyCode::Left | KeyCode::Right | KeyCode::Tab => { let t = 1 - self.shop_tab; self.open_shop(t); }
            KeyCode::Enter | KeyCode::Char('b') => self.buy(),
            KeyCode::F(5) => { let t = self.shop_tab; self.open_shop(t); }
            _ => {}
        }
    }

    // ---------- настройки ----------
    fn settings_key(&mut self, k: KeyEvent) {
        if let Some(buf) = &mut self.set_edit {
            match k.code {
                KeyCode::Char(c) => buf.push(c),
                KeyCode::Backspace => { buf.pop(); }
                KeyCode::Esc => self.set_edit = None,
                KeyCode::Enter => {
                    let v = buf.clone(); self.set_edit = None;
                    let key = SECTIONS[self.set_sec].items[self.set_item].key;
                    self.st.set(key, v); self.apply_setting(key);
                }
                _ => {}
            }
            return;
        }
        let nsec = SECTIONS.len();
        let nit = SECTIONS[self.set_sec].items.len();
        match k.code {
            KeyCode::Esc => { if self.set_col == 1 { self.set_col = 0 } else { self.page = Page::Chats } }
            KeyCode::Up => if self.set_col == 0 { self.set_sec = self.set_sec.saturating_sub(1); self.set_item = 0 } else { self.set_item = self.set_item.saturating_sub(1) },
            KeyCode::Down => if self.set_col == 0 { self.set_sec = (self.set_sec + 1).min(nsec - 1); self.set_item = 0 } else { self.set_item = (self.set_item + 1).min(nit - 1) },
            KeyCode::Right | KeyCode::Enter | KeyCode::Char(' ') => if self.set_col == 0 { self.set_col = 1 } else { self.activate(1) },
            KeyCode::Left => if self.set_col == 1 {
                if matches!(SECTIONS[self.set_sec].items[self.set_item].kind, SKind::Choice(_)) { self.activate(-1) } else { self.set_col = 0 }
            },
            _ => {}
        }
    }
    fn activate(&mut self, dir: i32) {
        let it = &SECTIONS[self.set_sec].items[self.set_item];
        match &it.kind {
            SKind::Toggle => { let v = !self.st.on(it.key); self.st.set(it.key, if v { "1" } else { "0" }.into()); self.apply_setting(it.key); }
            SKind::Choice(_) => { self.st.cycle(it.key, dir); self.apply_setting(it.key); }
            SKind::Text => self.set_edit = Some({ let v = self.st.get(it.key); if v.is_empty() { String::new() } else { v } }),
            SKind::Action => self.do_action(it.key),
            SKind::Info => {}
        }
    }
    fn do_action(&mut self, key: &str) {
        match key {
            "logout" => {
                if self.demo { self.screen = Screen::Login; self.login.step = LStep::Qr; self.login.qr = make_qr(&demo_link()); self.login.pass.clear(); }
                else {
                    self.cmd(Cmd::LogOut);
                    self.chats.clear(); self.tabs.clear(); self.sel = 0; self.sel_id = None; self.page = Page::Chats; self.screen = Screen::Login;
                    self.login = Login { step: LStep::Wait, phone: String::new(), code: String::new(), pass: String::new(), show: false, err: String::new(), qr: vec![],
                        api_id: self.login.api_id.clone(), api_hash: self.login.api_hash.clone(), field: 0, status: "Выход из аккаунта...".into(), hint: String::new(), code_hint: String::new(), qr_requested: false };
                }
            }
            "pl_rescan" => { let d = self.st.music_dir(); self.player.scan(&d); self.notice = format!("Найдено треков: {}", self.player.tracks.len()); }
            "sessions_open" => self.open_sessions(),
            "shop_premium" => self.open_shop(0),
            "shop_stars" => self.open_shop(1),
            "cloudpass" => self.notice = "Смена облачного пароля появится после подключения TDLib".into(),
            _ => self.notice = "Пока не реализовано (появится вместе с TDLib)".into(),
        }
    }
    fn apply_setting(&mut self, key: &str) {
        match key {
            "mouse" => self.mouse_changed = true,
            "pl_vol" => { let v = self.st.idx("pl_vol") as i32 * 25; self.player.set_volume(v); }
            "pl_shuffle" => self.player.shuffle = self.st.on("pl_shuffle"),
            "pl_repeat" => self.player.repeat = self.st.idx("pl_repeat") as u8,
            "music_dir" => { let d = self.st.music_dir(); self.player.scan(&d); }
            "ghost" | "ghost_offline" => { self.sync_online(); if key == "ghost" { self.notice = if self.ghost() { "[~] режим невидимки включён".into() } else { "Режим невидимки выключен".into() }; } }
            _ => {}
        }
    }

    // ---------- плеер ----------
    fn player_key(&mut self, k: KeyEvent) {
        let n = self.player.tracks.len();
        match k.code {
            KeyCode::Esc => self.page = Page::Chats,
            KeyCode::Up => self.player.cursor = self.player.cursor.saturating_sub(1),
            KeyCode::Down => if n > 0 { self.player.cursor = (self.player.cursor + 1).min(n - 1) },
            KeyCode::Enter => { let c = self.player.cursor; self.player.play_index(c, Duration::ZERO); }
            KeyCode::Char(' ') => self.player.toggle(),
            KeyCode::Char('n') => self.player.next(),
            KeyCode::Char('p') => self.player.prev(),
            KeyCode::Left => self.player.seek_rel(-5),
            KeyCode::Right => self.player.seek_rel(5),
            KeyCode::Char('+') | KeyCode::Char('=') => { let v = self.player.volume as i32 + 5; self.player.set_volume(v); }
            KeyCode::Char('-') => { let v = self.player.volume as i32 - 5; self.player.set_volume(v); }
            KeyCode::Char('s') => self.toggle_shuffle(),
            KeyCode::Char('r') => self.cycle_repeat(),
            KeyCode::F(5) => { let d = self.st.music_dir(); self.player.scan(&d); }
            _ => {}
        }
    }
    fn toggle_shuffle(&mut self) { self.player.shuffle = !self.player.shuffle; self.st.set("pl_shuffle", if self.player.shuffle { "1" } else { "0" }.into()); }
    fn cycle_repeat(&mut self) { self.player.repeat = (self.player.repeat + 1) % 3; self.st.set("pl_repeat", self.player.repeat.to_string()); }

    // ---------- мышь ----------
    pub fn mouse(&mut self, m: MouseEvent) {
        let (c, r) = (m.column, m.row);
        self.last_input = Instant::now();
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => self.click(c, r, false),
            MouseEventKind::Down(MouseButton::Right) => self.click(c, r, true),
            MouseEventKind::ScrollUp => self.wheel(c, r, -1),
            MouseEventKind::ScrollDown => self.wheel(c, r, 1),
            _ => {}
        }
    }
    fn hit_at(&self, c: u16, r: u16) -> Option<(Rect, Hit)> {
        self.hits.iter().rev().find(|(rc, _)| c >= rc.x && c < rc.x + rc.width && r >= rc.y && r < rc.y + rc.height).copied()
    }
    fn wheel(&mut self, c: u16, r: u16, d: i32) {
        if self.screen != Screen::Main || self.menu.is_some() || self.help || self.chat_menu.is_some() { return; }
        if self.page == Page::Sessions { let n = self.sessions.len() as i32; if n > 0 { self.ses_sel = (self.ses_sel as i32 + d).clamp(0, n - 1) as usize; } return; }
        let Some((_, h)) = self.hit_at(c, r) else { return };
        match h {
            Hit::Chat(_) | Hit::ListArea => {
                if c >= LEFT_W { return; }
                let n = self.visible().len();
                let maxoff = n.saturating_sub(self.list_cap);
                self.list_off = (self.list_off as i32 + d * 2).clamp(0, maxoff as i32) as usize;
            }
            Hit::Msg(_) | Hit::MsgArea => {
                self.msg_sel = None;
                self.scroll_up = (self.scroll_up as i32 - d * 3).clamp(0, self.max_scroll as i32) as usize;
            }
            Hit::SetSection(_) | Hit::SetItem(_) | Hit::SetArea => {
                if self.set_col == 0 { self.set_sec = (self.set_sec as i32 + d).clamp(0, SECTIONS.len() as i32 - 1) as usize; self.set_item = 0; }
                else { self.set_item = (self.set_item as i32 + d).clamp(0, SECTIONS[self.set_sec].items.len() as i32 - 1) as usize; }
            }
            Hit::PlTrack(_) | Hit::PlArea => {
                let n = self.player.tracks.len() as i32;
                if n > 0 { self.player.cursor = (self.player.cursor as i32 + d).clamp(0, n - 1) as usize; }
            }
            _ => {}
        }
    }
    fn click(&mut self, c: u16, r: u16, right: bool) {
        let hit = self.hit_at(c, r);
        if self.mouse_dbg { self.notice = format!("мышь: колонка {c}, строка {r} -> {:?}", hit.map(|x| x.1)); }
        if self.screen == Screen::Main {
            if self.help { self.help = false; return; }
            if let Some((ci, _, _)) = self.chat_menu {
                self.chat_menu = None;
                if let Some((_, Hit::ChatMenuItem(i))) = hit { self.chat_action_for(ci, i); }
                return;
            }
            if self.menu.is_some() {
                if let Some((_, Hit::MenuItem(i))) = hit { self.menu = None; self.menu_do(i); } else { self.menu = None; }
                return;
            }
        }
        let Some((rect, h)) = hit else { return };
        // Страховка: элементы левой колонки реагируют только на клики внутри неё.
        if matches!(h, Hit::Chat(_) | Hit::ListArea | Hit::Search | Hit::Folder(_)) && c >= LEFT_W { return; }
        match h {
            // вход
            Hit::LoginPhone => self.login_to_phone(),
            Hit::LoginApi => { self.login.step = LStep::Api; self.login.field = 0; self.login.err.clear(); }
            Hit::LoginScan | Hit::LoginNext => self.login_enter(),
            Hit::LoginBack => self.login_back(),
            Hit::LoginEye => self.login.show = !self.login.show,
            // общее
            Hit::OpenSettings => self.page = Page::Settings,
            Hit::OpenPlayer | Hit::MiniOpen => self.open_player(),
            Hit::Back => { if self.set_edit.is_some() { self.set_edit = None } else if matches!(self.page, Page::Sessions | Page::Shop) { self.page = Page::Settings } else { self.page = Page::Chats } }
            Hit::Ghost => { let v = !self.ghost(); self.st.set("ghost", if v { "1" } else { "0" }.into()); self.sync_online();
                self.notice = if v { "[~] режим невидимки включён".into() } else { "Режим невидимки выключен".into() }; }
            // список чатов
            Hit::Search => self.focus = Focus::Search,
            Hit::Folder(i) => {
                self.folder = i; self.sel = 0; self.list_off = 0; self.on_select();
                if i == 5 && !self.demo && !self.chats.iter().any(|c| c.saved) { self.cmd(Cmd::OpenSaved); }
                if i == 6 && !self.demo { self.cmd(Cmd::LoadChats { archive: true }); }
            }
            Hit::Chat(vi) => {
                if right {
                    if let Some(&ci) = self.visible().get(vi) { self.chat_menu = Some((ci, (c, r), 0)); }
                } else { self.sel = vi; self.on_select(); self.open_current_tab(); }
            }
            Hit::Tab(id) => self.select_chat_id(id),
            Hit::TabClose(id) => self.tabs.retain(|&t| t != id),
            // шапка и ввод
            Hit::HdrInfo => self.info = !self.info,
            Hit::HdrCall => self.notice = "Исходящие звонки недоступны: аудио-движок tgcalls не входит в TDLib. Входящие можно отклонить (F9)".into(),
            Hit::CallDecline => self.decline_call(),
            Hit::ChatMenuItem(_) => {}
            Hit::SesRow(i) => self.ses_sel = i,
            Hit::SesKill => self.session_act(0),
            Hit::SesKillAll => self.session_act(1),
            Hit::SesCalls => self.session_act(2),
            Hit::SesSecret => self.session_act(3),
            Hit::SesTtl => self.session_act(4),
            Hit::ShopTab(t) => self.open_shop(t),
            Hit::ShopBuy => self.buy(),
            Hit::Attach => self.start_attach(),
            Hit::HdrSearch | Hit::HdrMenu | Hit::Emoji | Hit::MicBtn => self.notice = "Пока не реализовано".into(),
            Hit::SendBtn => self.send(),
            Hit::InputBox => { self.ensure_open(); self.focus = Focus::Input; }
            Hit::BannerX => { self.reply = None; self.edit = None; self.input.clear(); }
            Hit::Msg(i) => {
                let Some(ch) = self.cur() else { return };
                if i >= self.chats[ch].msgs.len() { return; }
                self.focus = Focus::Messages;
                let was = self.msg_sel == Some(i);
                self.msg_sel = Some(i);
                if right { self.menu = Some(0); self.menu_pos = Some((c, r)); }
                else if let Some((li, t)) = self.last_click { if li == i && t.elapsed() < Duration::from_millis(450) && was { self.open_msg(ch, i); } }
                self.last_click = Some((i, Instant::now()));
            }
            // мини-плеер и плеер
            Hit::MiniPrev | Hit::PlPrev => self.player.prev(),
            Hit::MiniPlay | Hit::PlPlay => self.player.toggle(),
            Hit::MiniNext | Hit::PlNext => self.player.next(),
            Hit::MiniClose => self.player.stop(),
            Hit::MiniSeek | Hit::PlSeek => { let f = (c.saturating_sub(rect.x)) as f32 / rect.width.max(1) as f32; self.player.seek_frac(f); }
            Hit::PlShuffle => self.toggle_shuffle(),
            Hit::PlRepeat => self.cycle_repeat(),
            Hit::PlVolDown => { let v = self.player.volume as i32 - 10; self.player.set_volume(v); }
            Hit::PlVolUp => { let v = self.player.volume as i32 + 10; self.player.set_volume(v); }
            Hit::PlTrack(i) => {
                self.player.cursor = i;
                if self.player.cur == Some(i) && !right { self.player.toggle(); } else { self.player.play_index(i, Duration::ZERO); }
            }
            // настройки
            Hit::SetSection(i) => { self.set_sec = i; self.set_item = 0; self.set_col = 0; self.set_edit = None; }
            Hit::SetItem(i) => { self.set_item = i; self.set_col = 1; self.activate(if right { -1 } else { 1 }); }
            _ => {}
        }
    }
}
