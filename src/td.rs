// Связь с Telegram через TDLib (крейт tdlib-rs).
// Устройство: поток-приёмник вызывает tdlib_rs::receive() и передаёт обновления
// в поток-исполнитель (tokio, один поток). Тот переводит их в простые события Ev для интерфейса.
// Интерфейс шлёт команды Cmd и читает события через Backend::rx.
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    ffi::CString,
    os::raw::c_char,
    path::PathBuf,
    rc::Rc,
    sync::mpsc,
    thread,
    time::Duration,
};
use chrono::{Datelike, Local, TimeZone, Timelike};
use tdlib_rs::{enums, functions, types};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use crate::{avatar::Photo, data::{Kind, Msg}};

extern "C" {
    fn td_execute(request: *const c_char) -> *const c_char;
}
/// Синхронный запрос TDLib без клиента (настройка логов до создания клиента).
fn execute(req: &str) {
    if let Ok(c) = CString::new(req) {
        unsafe { let _ = td_execute(c.as_ptr()); }
    }
}

// ---------- публичный интерфейс модуля ----------
pub enum Cmd {
    RequestQr,
    Phone(String),
    Code(String),
    Password(String),
    Restart,
    LogOut,
    Close,
    SetOnline(bool),
    LoadChats { archive: bool },
    Avatar { key: i64, file: i32 },
    SendFile { chat: i64, path: String, reply: Option<i64> },
    Pin { chat: i64, pin: bool },
    Archive { chat: i64, archive: bool },
    OpenSaved,
    EnsureSaved,
    Sessions,
    KillSession(i64),
    KillOthers,
    SessionTtl(i32),
    SessionCalls { id: i64, on: bool },
    SessionSecret { id: i64, on: bool },
    Premium,
    Stars,
    DeclineCall(i32),
    History(i64),
    Send { chat: i64, text: String, reply: Option<i64> },
    Edit { chat: i64, msg: i64, text: String },
    Delete { chat: i64, ids: Vec<i64> },
    View { chat: i64, ids: Vec<i64> },
    OpenContent { chat: i64, msg: i64 },
    Download { file: i32, name: String, open: bool },
}

pub enum Auth {
    WaitPhone,
    Qr(String),
    WaitCode { phone: String, via: String },
    WaitPassword { hint: String },
    Ready,
    Closed,
    Unsupported(String),
}

pub struct ChatInfo {
    pub id: i64, pub title: String, pub kind: char, pub status: String, pub unread: i32,
    pub muted: bool, pub order: i64, pub pinned: bool, pub last: Option<Msg>, pub user_id: i64, pub read_out: i64,
    pub arch_order: i64, pub photo: Option<i32>, pub saved: bool,
}

pub struct SessionInfo {
    pub id: i64, pub current: bool, pub unconfirmed: bool, pub calls: bool, pub secret: bool, pub official: bool,
    pub app: String, pub device: String, pub system: String, pub location: String, pub ip: String, pub last_active: String,
}

pub struct CallInfo { pub id: i32, pub user: String, pub incoming: bool, pub video: bool, pub state: String }

pub enum Ev {
    Auth(Auth),
    Fail(String),
    ApiInvalid,
    ChatNew(ChatInfo),
    ChatLast { id: i64, last: Option<Msg>, order: i64, pinned: bool, arch_order: i64 },
    ChatPos { id: i64, archive: bool, order: i64, pinned: bool },
    ChatUnread { id: i64, count: i32 },
    ChatTitle { id: i64, title: String },
    ChatReadOut { id: i64, msg: i64 },
    ChatMute { id: i64, muted: bool },
    UserStatus { user_id: i64, status: String },
    NewMsg(i64, Msg),
    MsgContent { chat: i64, id: i64, text: String, kind: Kind, file: Option<i32>, listened: bool },
    Replaced { chat: i64, old: i64, msg: Msg },
    Deleted { chat: i64, ids: Vec<i64> },
    History { chat: i64, msgs: Vec<Msg> },
    Downloaded { path: String, name: String, open: bool },
    Me { id: i64, name: String, username: String, phone: String, premium: bool, photo: Option<i32> },
    Avatar { key: i64, photo: Photo },
    OpenChat(i64),
    Sessions { list: Vec<SessionInfo>, ttl: i32 },
    Premium { text: String, options: Vec<String> },
    Stars { balance: i64, options: Vec<String> },
    StarBalance(i64),
    Call { info: CallInfo, ended: bool },
}

enum Inner { Cmd(Cmd), Upd(enums::Update, i32) }

pub struct Backend { tx: UnboundedSender<Inner>, pub rx: mpsc::Receiver<Ev> }

impl Backend {
    pub fn start(api_id: i32, api_hash: String, dir: PathBuf, lowmem: bool) -> Backend {
        let _ = std::fs::create_dir_all(&dir);
        // Логи TDLib только в файл и только важное: иначе они испортят экран терминала.
        let log = serde_json::json!({"@type": "setLogStream", "log_stream": {"@type": "logStreamFile",
            "path": dir.join("tdlib.log").to_string_lossy(), "max_file_size": 2_000_000, "redirect_stderr": true}});
        execute(&log.to_string());
        execute(r#"{"@type":"setLogVerbosityLevel","new_verbosity_level":1}"#);

        let (tx, rx) = unbounded_channel::<Inner>();
        let (etx, erx) = mpsc::channel::<Ev>();
        let tx_recv = tx.clone();
        thread::spawn(move || loop {
            if let Some((u, c)) = tdlib_rs::receive() {
                if tx_recv.send(Inner::Upd(u, c)).is_err() { break; }
            }
        });
        thread::spawn(move || {
            let st = Rc::new(St { cid: Cell::new(0), users: RefCell::new(HashMap::new()), titles: RefCell::new(HashMap::new()),
                quit: Cell::new(false), api_id, api_hash, dir, me: Cell::new(0), lowmem });
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
            let local = tokio::task::LocalSet::new();
            local.block_on(&rt, run(rx, etx, st));
        });
        Backend { tx, rx: erx }
    }
    pub fn send(&self, c: Cmd) { let _ = self.tx.send(Inner::Cmd(c)); }
}

// ---------- исполнитель ----------
struct St {
    cid: Cell<i32>,
    users: RefCell<HashMap<i64, String>>,
    titles: RefCell<HashMap<i64, String>>,
    quit: Cell<bool>,
    api_id: i32,
    api_hash: String,
    dir: PathBuf,
    me: Cell<i64>,
    lowmem: bool,
}
type S = Rc<St>;
type Tx = mpsc::Sender<Ev>;

fn start_client(st: &S) {
    let cid = tdlib_rs::create_client();
    st.cid.set(cid);
    // Любой первый запрос «будит» клиент, после него приходят обновления.
    tokio::task::spawn_local(async move { let _ = functions::set_log_verbosity_level(1, cid).await; });
}

async fn run(mut rx: UnboundedReceiver<Inner>, etx: Tx, st: S) {
    start_client(&st);
    let (utx, mut urx) = unbounded_channel::<(enums::Update, i32)>();
    {
        let (st, etx) = (st.clone(), etx.clone());
        // Обновления обрабатываются строго по очереди, чтобы порядок событий сохранялся.
        tokio::task::spawn_local(async move {
            while let Some((u, c)) = urx.recv().await {
                if c != st.cid.get() { continue; }
                on_update(u, &st, &etx).await;
            }
        });
    }
    while let Some(m) = rx.recv().await {
        match m {
            Inner::Upd(u, c) => { let _ = utx.send((u, c)); }
            Inner::Cmd(c) => {
                let (st, etx) = (st.clone(), etx.clone());
                tokio::task::spawn_local(async move { on_cmd(c, &st, &etx).await; });
            }
        }
    }
}

fn tr_err(e: &types::Error) -> String {
    let m = e.message.as_str();
    if m.contains("PHONE_NUMBER_INVALID") { "Неверный номер телефона".into() }
    else if m.contains("PHONE_CODE_INVALID") { "Неверный код".into() }
    else if m.contains("PHONE_CODE_EXPIRED") { "Код устарел, вернитесь назад и запросите новый".into() }
    else if m.contains("PASSWORD_HASH_INVALID") { "Неверный пароль".into() }
    else if m.contains("FLOOD_WAIT") || m.contains("Too Many Requests") { format!("Слишком много попыток, подождите ({m})") }
    else { format!("Ошибка {}: {}", e.code, m) }
}
fn fail(etx: &Tx, e: &types::Error) {
    if e.message.contains("API_ID_INVALID") || e.message.contains("API_ID_PUBLISHED_FLOOD") { let _ = etx.send(Ev::ApiInvalid); }
    else { let _ = etx.send(Ev::Fail(tr_err(e))); }
}

async fn on_cmd(c: Cmd, st: &S, etx: &Tx) {
    let cid = st.cid.get();
    macro_rules! chk { ($e:expr) => { if let Err(e) = $e.await { fail(etx, &e); } }; }
    match c {
        Cmd::RequestQr => chk!(functions::request_qr_code_authentication(vec![], cid)),
        Cmd::Phone(p) => chk!(functions::set_authentication_phone_number(p, None, cid)),
        Cmd::Code(c) => chk!(functions::check_authentication_code(c, cid)),
        Cmd::Password(p) => chk!(functions::check_authentication_password(p, cid)),
        Cmd::Restart => { let _ = functions::close(cid).await; }
        Cmd::LogOut => chk!(functions::log_out(cid)),
        Cmd::Close => { st.quit.set(true); let _ = functions::close(cid).await; }
        Cmd::SetOnline(v) => {
            let val = enums::OptionValue::Boolean(types::OptionValueBoolean { value: v });
            let _ = functions::set_option("online".to_string(), Some(val), cid).await;
        }
        Cmd::LoadChats { archive } => {
            let list = if archive { Some(enums::ChatList::Archive) } else { None };
            let _ = functions::load_chats(list, 40, cid).await; // порциями, по мере прокрутки списка
        }
        Cmd::Avatar { key, file } => {
            if let Ok(enums::File::File(f)) = functions::download_file(file, 1, 0, 0, true, cid).await {
                if !f.local.path.is_empty() { if let Some(photo) = decode_avatar(&f.local.path) { let _ = etx.send(Ev::Avatar { key, photo }); } }
            }
        }
        Cmd::SendFile { chat, path, reply } => {
            let reply_to = reply.map(|id| enums::InputMessageReplyTo::Message(types::InputMessageReplyToMessage { message_id: id, quote: None, checklist_task_id: 0 }));
            let file = enums::InputFile::Local(types::InputFileLocal { path: path.clone() });
            let low = path.to_lowercase();
            let content = if [".jpg", ".jpeg", ".png"].iter().any(|e| low.ends_with(e)) {
                enums::InputMessageContent::InputMessagePhoto(types::InputMessagePhoto { photo: file, thumbnail: None, added_sticker_file_ids: vec![],
                    width: 0, height: 0, caption: None, show_caption_above_media: false, self_destruct_type: None, has_spoiler: false })
            } else {
                enums::InputMessageContent::InputMessageDocument(types::InputMessageDocument { document: file, thumbnail: None, disable_content_type_detection: false, caption: None })
            };
            chk!(functions::send_message(chat, None, reply_to, None, content, cid))
        }
        Cmd::Pin { chat, pin } => chk!(functions::toggle_chat_is_pinned(enums::ChatList::Main, chat, pin, cid)),
        Cmd::Archive { chat, archive } => chk!(functions::add_chat_to_list(chat, if archive { enums::ChatList::Archive } else { enums::ChatList::Main }, cid)),
        Cmd::OpenSaved => {
            let me = st.me.get();
            if me != 0 { match functions::create_private_chat(me, false, cid).await { Ok(enums::Chat::Chat(c)) => { let _ = etx.send(Ev::OpenChat(c.id)); } Err(e) => fail(etx, &e) } }
        }
        Cmd::EnsureSaved => { let me = st.me.get(); if me != 0 { let _ = functions::create_private_chat(me, false, cid).await; } }
        Cmd::Sessions => send_sessions(cid, etx).await,
        Cmd::KillSession(id) => { chk!(functions::terminate_session(id, cid)); send_sessions(cid, etx).await; }
        Cmd::KillOthers => { chk!(functions::terminate_all_other_sessions(cid)); send_sessions(cid, etx).await; }
        Cmd::SessionTtl(d) => { chk!(functions::set_inactive_session_ttl(d, cid)); send_sessions(cid, etx).await; }
        Cmd::SessionCalls { id, on } => { chk!(functions::toggle_session_can_accept_calls(id, on, cid)); send_sessions(cid, etx).await; }
        Cmd::SessionSecret { id, on } => { chk!(functions::toggle_session_can_accept_secret_chats(id, on, cid)); send_sessions(cid, etx).await; }
        Cmd::Premium => {
            match functions::get_premium_state(cid).await {
                Ok(enums::PremiumState::PremiumState(p)) => {
                    let options = p.payment_options.iter().map(|o| {
                        let po = &o.payment_option;
                        format!("{} мес. за {:.2} {}{}{}", po.month_count, po.amount as f64 / 100.0, po.currency,
                            if po.discount_percentage > 0 { format!(" (скидка {}%)", po.discount_percentage) } else { String::new() },
                            if o.is_current { " [текущий]" } else { "" })
                    }).collect();
                    let _ = etx.send(Ev::Premium { text: clean(p.state.text), options });
                }
                Err(e) => fail(etx, &e),
            }
        }
        Cmd::Stars => {
            let opts = match functions::get_star_payment_options(cid).await {
                Ok(enums::StarPaymentOptions::StarPaymentOptions(o)) => o.options.iter().map(|x| format!("{} звёзд за {:.2} {}", x.star_count, x.amount as f64 / 100.0, x.currency)).collect(),
                Err(e) => { fail(etx, &e); vec![] }
            };
            let owner = enums::MessageSender::User(types::MessageSenderUser { user_id: st.me.get() });
            let bal = match functions::get_star_transactions(owner, String::new(), None, String::new(), 1, cid).await {
                Ok(enums::StarTransactions::StarTransactions(t)) => t.star_amount.star_count,
                Err(_) => 0,
            };
            let _ = etx.send(Ev::Stars { balance: bal, options: opts });
        }
        Cmd::DeclineCall(id) => chk!(functions::discard_call(id, false, String::new(), 0, false, 0, cid)),
        Cmd::History(chat) => {
            let mut best: Vec<Msg> = vec![];
            for attempt in 0..2 {
                if let Ok(enums::Messages::Messages(m)) = functions::get_chat_history(chat, 0, 0, 30, false, cid).await {
                    let mut v = vec![];
                    for x in m.messages.into_iter().flatten() { v.push(conv_msg(x, st).await); }
                    if v.len() > best.len() { best = v; }
                }
                if best.len() >= 30 || attempt == 1 { break; }
                tokio::time::sleep(Duration::from_millis(500)).await; // первый ответ бывает неполным
            }
            best.sort_by_key(|m| m.id);
            let _ = etx.send(Ev::History { chat, msgs: best });
        }
        Cmd::Send { chat, text, reply } => {
            let reply_to = reply.map(|id| enums::InputMessageReplyTo::Message(types::InputMessageReplyToMessage { message_id: id, quote: None, checklist_task_id: 0 }));
            chk!(functions::send_message(chat, None, reply_to, None, text_content(text), cid))
        }
        Cmd::Edit { chat, msg, text } => chk!(functions::edit_message_text(chat, msg, text_content(text), cid)),
        Cmd::Delete { chat, ids } => chk!(functions::delete_messages(chat, ids, true, cid)),
        Cmd::View { chat, ids } => chk!(functions::view_messages(chat, ids, Some(enums::MessageSource::ChatHistory), true, cid)),
        Cmd::OpenContent { chat, msg } => { let _ = functions::open_message_content(chat, msg, cid).await; }
        Cmd::Download { file, name, open } => {
            match functions::download_file(file, 32, 0, 0, true, cid).await {
                Ok(enums::File::File(f)) if !f.local.path.is_empty() => { let _ = etx.send(Ev::Downloaded { path: f.local.path, name, open }); }
                Ok(_) => { let _ = etx.send(Ev::Fail("Файл не удалось скачать".into())); }
                Err(e) => fail(etx, &e),
            }
        }
    }
}

async fn send_sessions(cid: i32, etx: &Tx) {
    match functions::get_active_sessions(cid).await {
        Ok(enums::Sessions::Sessions(s)) => {
            let list = s.sessions.into_iter().map(|x| {
                let d = local(x.last_active_date);
                SessionInfo { id: x.id, current: x.is_current, unconfirmed: x.is_unconfirmed, calls: x.can_accept_calls, secret: x.can_accept_secret_chats,
                    official: x.is_official_application, app: format!("{} {}", x.application_name, x.application_version).trim().to_string(),
                    device: x.device_model, system: format!("{} {}", x.platform, x.system_version).trim().to_string(), location: x.location, ip: x.ip_address,
                    last_active: format!("{:02}.{:02}.{} {:02}:{:02}", d.day(), d.month(), d.year(), d.hour(), d.minute()) }
            }).collect();
            let _ = etx.send(Ev::Sessions { list, ttl: s.inactive_session_ttl_days });
        }
        Err(e) => fail(etx, &e),
    }
}

/// Аватарка 4x4 пикселя для рисования полублоками.
fn decode_avatar(path: &str) -> Option<Photo> {
    let img = image::open(path).ok()?.resize_exact(4, 4, image::imageops::FilterType::Triangle).to_rgb8();
    let mut out = [[(0u8, 0u8, 0u8); 4]; 4];
    for y in 0..4 { for x in 0..4 { let p = img.get_pixel(x as u32, y as u32); out[y][x] = (p[0], p[1], p[2]); } }
    Some(out)
}

fn text_content(text: String) -> enums::InputMessageContent {
    enums::InputMessageContent::InputMessageText(types::InputMessageText {
        text: types::FormattedText { text, entities: vec![] }, link_preview_options: None, clear_draft: true })
}

fn os_name() -> String { if cfg!(windows) { "Windows".into() } else if cfg!(target_os = "macos") { "macOS".into() } else { "Linux".into() } }

async fn on_update(u: enums::Update, st: &S, etx: &Tx) {
    use enums::Update as U;
    match u {
        U::AuthorizationState(a) => auth(a.authorization_state, st, etx).await,
        U::NewChat(n) => { let ci = conv_chat(n.chat, st).await; let _ = etx.send(Ev::ChatNew(ci)); }
        U::ChatLastMessage(x) => {
            let last = match x.last_message { Some(m) => Some(conv_msg(m, st).await), None => None };
            let (order, pinned) = list_pos(&x.positions, false);
            let (arch_order, _) = list_pos(&x.positions, true);
            let _ = etx.send(Ev::ChatLast { id: x.chat_id, last, order, pinned, arch_order });
        }
        U::ChatPosition(x) => {
            match x.position.list {
                enums::ChatList::Main => { let _ = etx.send(Ev::ChatPos { id: x.chat_id, archive: false, order: x.position.order, pinned: x.position.is_pinned }); }
                enums::ChatList::Archive => { let _ = etx.send(Ev::ChatPos { id: x.chat_id, archive: true, order: x.position.order, pinned: false }); }
                _ => {}
            }
        }
        U::ChatReadInbox(x) => { let _ = etx.send(Ev::ChatUnread { id: x.chat_id, count: x.unread_count }); }
        U::ChatReadOutbox(x) => { let _ = etx.send(Ev::ChatReadOut { id: x.chat_id, msg: x.last_read_outbox_message_id }); }
        U::ChatTitle(x) => { st.titles.borrow_mut().insert(x.chat_id, x.title.clone()); let _ = etx.send(Ev::ChatTitle { id: x.chat_id, title: x.title }); }
        U::ChatNotificationSettings(x) => { let _ = etx.send(Ev::ChatMute { id: x.chat_id, muted: x.notification_settings.mute_for > 0 }); }
        U::NewMessage(x) => { let chat = x.message.chat_id; let m = conv_msg(x.message, st).await; let _ = etx.send(Ev::NewMsg(chat, m)); }
        U::MessageContent(x) => {
            let c = conv_content(&x.new_content);
            let _ = etx.send(Ev::MsgContent { chat: x.chat_id, id: x.message_id, text: c.text, kind: c.kind, file: c.file, listened: c.listened });
        }
        U::MessageSendSucceeded(x) => {
            let chat = x.message.chat_id; let m = conv_msg(x.message, st).await;
            let _ = etx.send(Ev::Replaced { chat, old: x.old_message_id, msg: m });
        }
        U::DeleteMessages(x) => { if x.is_permanent { let _ = etx.send(Ev::Deleted { chat: x.chat_id, ids: x.message_ids }); } }
        U::OwnedStarCount(x) => { let _ = etx.send(Ev::StarBalance(x.star_amount.star_count)); }
        U::User(x) => { if x.user.id == st.me.get() { let _ = etx.send(me_event(&x.user)); } }
        U::Call(x) => {
            let c = x.call;
            let user = user_name(c.user_id, st).await;
            let (state, ended) = match &c.state {
                enums::CallState::Pending(_) => ("вызов...".to_string(), false),
                enums::CallState::ExchangingKeys | enums::CallState::Ready(_) => ("соединение".to_string(), false),
                enums::CallState::HangingUp => ("завершение".to_string(), true),
                enums::CallState::Discarded(_) => ("завершён".to_string(), true),
                enums::CallState::Error(_) => ("ошибка".to_string(), true),
            };
            let _ = etx.send(Ev::Call { info: CallInfo { id: c.id, user, incoming: !c.is_outgoing, video: c.is_video, state }, ended });
        }
        U::UserStatus(x) => { let _ = etx.send(Ev::UserStatus { user_id: x.user_id, status: status_text(&x.status) }); }
        _ => {}
    }
}

async fn auth(s: enums::AuthorizationState, st: &S, etx: &Tx) {
    use enums::AuthorizationState as A;
    let cid = st.cid.get();
    match s {
        A::WaitTdlibParameters => {
            let db = st.dir.join("db").to_string_lossy().into_owned();
            let files = st.dir.join("files").to_string_lossy().into_owned();
            let r = functions::set_tdlib_parameters(false, db, files, String::new(), !st.lowmem, true, !st.lowmem, false, st.api_id,
                st.api_hash.clone(), "ru".into(), "echoping".into(), os_name(), env!("CARGO_PKG_VERSION").into(), cid).await;
            if let Err(e) = r { fail(etx, &e); }
        }
        A::WaitPhoneNumber => { let _ = etx.send(Ev::Auth(Auth::WaitPhone)); }
        A::WaitOtherDeviceConfirmation(x) => { let _ = etx.send(Ev::Auth(Auth::Qr(x.link))); }
        A::WaitCode(x) => {
            let via = match x.code_info.r#type {
                enums::AuthenticationCodeType::TelegramMessage(_) => "в приложение Telegram",
                enums::AuthenticationCodeType::Sms(_) => "по SMS",
                enums::AuthenticationCodeType::Call(_) => "звонком",
                _ => "способом, выбранным Telegram",
            };
            let _ = etx.send(Ev::Auth(Auth::WaitCode { phone: x.code_info.phone_number, via: via.into() }));
        }
        A::WaitPassword(x) => { let _ = etx.send(Ev::Auth(Auth::WaitPassword { hint: x.password_hint })); }
        A::Ready => {
            // Опции, уменьшающие расход памяти и трафика.
            for name in ["ignore_inline_thumbnails", "disable_top_chats", "disable_persistent_network_statistics", "use_storage_optimizer"] {
                let _ = functions::set_option(name.to_string(), Some(enums::OptionValue::Boolean(types::OptionValueBoolean { value: true })), cid).await;
            }
            if let Ok(enums::User::User(u)) = functions::get_me(cid).await {
                st.me.set(u.id);
                st.users.borrow_mut().insert(u.id, "Вы".to_string());
                let _ = etx.send(me_event(&u));
            }
            let _ = etx.send(Ev::Auth(Auth::Ready));
        }
        A::Closed => {
            let _ = etx.send(Ev::Auth(Auth::Closed));
            if !st.quit.get() { start_client(st); } // перезапуск после выхода из аккаунта или смены способа входа
        }
        A::LoggingOut | A::Closing => {}
        A::WaitRegistration(_) => { let _ = etx.send(Ev::Auth(Auth::Unsupported("Этот номер не зарегистрирован в Telegram. Зарегистрируйтесь в официальном приложении.".into()))); }
        _ => { let _ = etx.send(Ev::Auth(Auth::Unsupported("Этот способ входа (e-mail / покупка Premium) пока не поддерживается.".into()))); }
    }
}

// ---------- преобразование типов TDLib ----------
fn local(ts: i32) -> chrono::DateTime<Local> { Local.timestamp_opt(ts as i64, 0).single().unwrap_or_else(Local::now) }

const MONTHS: [&str; 12] = ["января", "февраля", "марта", "апреля", "мая", "июня", "июля", "августа", "сентября", "октября", "ноября", "декабря"];

fn hsize(b: i64) -> String {
    let b = b.max(0) as f64;
    if b >= 1_048_576.0 { format!("{:.1} МБ", b / 1_048_576.0) } else if b >= 1024.0 { format!("{:.0} КБ", b / 1024.0) } else { format!("{b:.0} Б") }
}
fn dur(s: i32) -> String { format!("{}:{:02}", s / 60, s % 60) }

fn status_text(s: &enums::UserStatus) -> String {
    use enums::UserStatus as U;
    match s {
        U::Online(_) => "в сети".into(),
        U::Offline(o) => { let t = local(o.was_online); format!("был(а) {:02}.{:02} в {:02}:{:02}", t.day(), t.month(), t.hour(), t.minute()) }
        U::Recently(_) => "был(а) недавно".into(),
        U::LastWeek(_) => "был(а) на этой неделе".into(),
        U::LastMonth(_) => "был(а) в этом месяце".into(),
        U::Empty => "был(а) давно".into(),
    }
}

fn list_pos(p: &[types::ChatPosition], archive: bool) -> (i64, bool) {
    p.iter().find(|x| if archive { matches!(x.list, enums::ChatList::Archive) } else { matches!(x.list, enums::ChatList::Main) })
        .map(|x| (x.order, x.is_pinned)).unwrap_or((0, false))
}

fn me_event(u: &types::User) -> Ev {
    Ev::Me {
        id: u.id, name: format!("{} {}", u.first_name, u.last_name).trim().to_string(),
        username: u.usernames.as_ref().and_then(|x| x.active_usernames.first().cloned()).unwrap_or_default(),
        phone: u.phone_number.clone(), premium: u.is_premium, photo: u.profile_photo.as_ref().map(|p| p.small.id),
    }
}

struct Conv { text: String, kind: Kind, file: Option<i32>, listened: bool }

fn conv_content(c: &enums::MessageContent) -> Conv {
    use enums::MessageContent as M;
    let mk = |text: String, kind: Kind, file: Option<i32>, listened: bool| Conv { text, kind, file, listened };
    match c {
        M::MessageText(t) => mk(t.text.text.clone(), Kind::Text, None, true),
        M::MessagePhoto(p) => {
            let f = p.photo.sizes.last().map(|s| (s.photo.id, s.photo.size.max(s.photo.expected_size)));
            mk(p.caption.text.clone(), Kind::Image("фото.jpg".into(), f.map(|x| hsize(x.1)).unwrap_or_default()), f.map(|x| x.0), true)
        }
        M::MessageVideo(v) => {
            let n = if v.video.file_name.is_empty() { "видео.mp4".to_string() } else { v.video.file_name.clone() };
            mk(v.caption.text.clone(), Kind::File(n, hsize(v.video.video.size.max(v.video.video.expected_size))), Some(v.video.video.id), true)
        }
        M::MessageVideoNote(v) => mk(String::new(), Kind::Video(dur(v.video_note.duration)), Some(v.video_note.video.id), v.is_viewed),
        M::MessageVoiceNote(v) => mk(v.caption.text.clone(), Kind::Voice(dur(v.voice_note.duration)), Some(v.voice_note.voice.id), v.is_listened),
        M::MessageDocument(d) => mk(d.caption.text.clone(), Kind::File(d.document.file_name.clone(), hsize(d.document.document.size.max(d.document.document.expected_size))), Some(d.document.document.id), true),
        M::MessageAudio(a) => {
            let n = if a.audio.file_name.is_empty() { "аудио.mp3".to_string() } else { a.audio.file_name.clone() };
            mk(a.caption.text.clone(), Kind::File(n, hsize(a.audio.audio.size.max(a.audio.audio.expected_size))), Some(a.audio.audio.id), true)
        }
        M::MessageAnimation(a) => {
            let n = if a.animation.file_name.is_empty() { "анимация.mp4".to_string() } else { a.animation.file_name.clone() };
            mk(a.caption.text.clone(), Kind::File(n, hsize(a.animation.animation.size.max(a.animation.animation.expected_size))), Some(a.animation.animation.id), true)
        }
        M::MessageSticker(_) => mk("[стикер]".into(), Kind::Text, None, true),
        M::MessageCall(c) => mk(String::new(), Kind::Call(format!("{}, {}", if c.is_video { "видео" } else { "аудио" }, dur(c.duration))), None, true),
        _ => mk("[сообщение этого типа пока не поддерживается]".into(), Kind::Text, None, true),
    }
}

async fn user_name(id: i64, st: &S) -> String {
    if let Some(n) = st.users.borrow().get(&id) { return n.clone(); }
    let name = match functions::get_user(id, st.cid.get()).await {
        Ok(enums::User::User(u)) => format!("{} {}", u.first_name, u.last_name).trim().to_string(),
        Err(_) => String::new(),
    };
    let name = if name.is_empty() { "Пользователь".to_string() } else { name };
    st.users.borrow_mut().insert(id, name.clone());
    name
}

fn clean(s: String) -> String { s.replace('\t', " ").chars().filter(|c| *c == '\n' || !c.is_control()).collect() }

async fn conv_msg(m: types::Message, st: &S) -> Msg {
    let author = if m.is_outgoing { "Вы".to_string() } else {
        match &m.sender_id {
            enums::MessageSender::User(u) => user_name(u.user_id, st).await,
            enums::MessageSender::Chat(c) => st.titles.borrow().get(&c.chat_id).cloned().unwrap_or_else(|| "Канал".into()),
        }
    };
    let c = conv_content(&m.content);
    let reply = match &m.reply_to {
        Some(enums::MessageReplyTo::Message(r)) => Some(match &r.content {
            Some(rc) => { let t = conv_content(rc); let t = if t.text.is_empty() { "[вложение]".to_string() } else { t.text.replace('\n', " ") }; t }
            None => "сообщение".to_string(),
        }),
        _ => None,
    };
    let d = local(m.date);
    Msg {
        id: m.id, file: c.file, out: m.is_outgoing, author,
        day: format!("{} {} {} г.", d.day(), MONTHS[(d.month0() as usize) % 12], d.year()),
        time: format!("{:02}:{:02}", d.hour(), d.minute()),
        text: clean(c.text), kind: c.kind, reply: reply.map(clean), edited: m.edit_date > 0, read: false,
        listened: m.is_outgoing || c.listened,
    }
}

async fn conv_chat(c: types::Chat, st: &S) -> ChatInfo {
    st.titles.borrow_mut().insert(c.id, c.title.clone());
    let (mut kind, mut user_id) = match &c.r#type {
        enums::ChatType::Private(p) => ('p', p.user_id),
        enums::ChatType::Secret(s) => ('p', s.user_id),
        enums::ChatType::BasicGroup(_) => ('g', 0),
        enums::ChatType::Supergroup(s) => (if s.is_channel { 'c' } else { 'g' }, 0),
    };
    let mut status = match kind { 'g' => "группа".to_string(), 'c' => "канал".to_string(), _ => String::new() };
    if user_id != 0 {
        if let Ok(enums::User::User(u)) = functions::get_user(user_id, st.cid.get()).await {
            status = status_text(&u.status);
            if matches!(u.r#type, enums::UserType::Bot(_)) { kind = 'b'; status = "бот".into(); }
        } else { user_id = 0; }
    }
    let (order, pinned) = list_pos(&c.positions, false);
    let (arch_order, _) = list_pos(&c.positions, true);
    let saved = user_id != 0 && user_id == st.me.get();
    let title = if saved { "Избранное".to_string() } else { clean(c.title) };
    let last = match c.last_message { Some(m) => Some(conv_msg(m, st).await), None => None };
    if saved { status = "заметки для себя".into(); }
    ChatInfo { id: c.id, title, kind, status, unread: c.unread_count, muted: c.notification_settings.mute_for > 0,
        order, pinned, last, user_id, read_out: c.last_read_outbox_message_id, arch_order, photo: c.photo.as_ref().map(|p| p.small.id), saved }
}
