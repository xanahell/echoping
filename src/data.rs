// Модель данных. Сейчас заполняется фейковыми данными (mock_chats);
// при подключении TDLib эти структуры заполняются из обновлений TDLib.

#[derive(Clone)]
pub enum Kind {
    Text,
    Image(String, String),
    File(String, String),
    Voice(String),
    Video(String), // видеосообщение («кружок»)
    Call(String),
}

#[derive(Clone)]
pub struct Msg {
    pub id: i64,
    pub sender: i64, // номер пользователя-отправителя (0 если неизвестен)
    pub file: Option<i32>, // id файла в TDLib (для скачивания)
    pub thumb: Option<i32>, // id миниатюры (видео, видеосообщения)
    pub out: bool,
    pub author: String,
    pub day: String,
    pub time: String,
    pub text: String,
    pub kind: Kind,
    pub reply: Option<String>,
    pub edited: bool,
    pub read: bool,
    pub listened: bool, // для голосовых и видеосообщений
}

#[derive(Default)]
pub struct Chat {
    pub id: i64,
    pub order: i64,     // позиция в основном списке (0 = не показывать)
    pub user_id: i64,   // для личных чатов
    pub read_out: i64,  // последнее прочитанное собеседником наше сообщение
    pub loaded: bool,   // история уже запрошена
    pub arch_order: i64, // позиция в архиве (0 = не в архиве)
    pub photo: Option<i32>, // файл аватарки в TDLib
    pub saved: bool,    // чат «Избранное» (сохранённые сообщения)
    pub read_only: bool, // писать нельзя (например, подписчик канала)
    pub sg_id: i64,     // номер супергруппы / канала в TDLib
    pub bg_id: i64,     // номер обычной группы в TDLib
    pub name: String,
    pub kind: char, // p личный, g группа, c канал, b бот
    pub status: String,
    pub unread: u32,
    pub pinned: bool,
    pub muted: bool,
    pub msgs: Vec<Msg>,
}

impl Chat {
    pub fn icon(&self) -> char {
        match self.kind { 'g' => '&', 'c' => '#', 'b' => '%', _ => ' ' }
    }
    pub fn preview(&self) -> String {
        match self.msgs.last() {
            None => String::new(),
            Some(m) => {
                let body = match &m.kind {
                    Kind::Text => m.text.replace('\n', " "),
                    Kind::Image(..) => "[фото]".into(),
                    Kind::File(n, _) => format!("[файл] {n}"),
                    Kind::Voice(d) => format!("[голосовое] {d}"),
                    Kind::Video(d) => format!("[видеосообщение] {d}"),
                    Kind::Call(_) => "[звонок]".into(),
                };
                if m.out { format!("Вы: {body}") }
                else if self.kind == 'g' { format!("{}: {body}", m.author) }
                else { body }
            }
        }
    }
}

pub const D1: &str = "12 ноября 2026 г.";
pub const D2: &str = "13 ноября 2026 г.";

pub fn msg(out: bool, author: &str, day: &str, time: &str, text: &str) -> Msg {
    Msg { id: 0, sender: 0, file: None, thumb: None, out, author: author.into(), day: day.into(), time: time.into(), text: text.into(),
          kind: Kind::Text, reply: None, edited: false, read: true, listened: true }
}
fn with(mut m: Msg, k: Kind) -> Msg {
    m.listened = m.out || !matches!(k, Kind::Voice(_) | Kind::Video(_));
    m.kind = k;
    m
}
fn rep(mut m: Msg, r: &str) -> Msg { m.reply = Some(r.into()); m }

pub fn mock_chats() -> Vec<Chat> {
    let mut v = vec![
        Chat { name: "Избранное".into(), kind: 'p', status: "заметки для себя".into(), unread: 0, pinned: true, muted: false, saved: true,
            msgs: vec![
                msg(true, "Вы", D1, "18:02", "купить кабель usb-c"),
                with(msg(true, "Вы", D2, "09:15", ""), Kind::File("конспект.pdf".into(), "1.2 МБ".into())),
            ], ..Default::default() },
        Chat { name: "perc057".into(), kind: 'p', status: "в сети".into(), unread: 3, pinned: true, muted: false,
            msgs: vec![
                msg(false, "perc057", D1, "22:04", "ну он долбоёб\nче я поделаю"),
                msg(true, "Вы", D1, "22:10", "перезайди"),
                rep(msg(false, "perc057", D2, "00:07", "я сам не понимаю, у меня есть инет"), "перезайди"),
                with(msg(false, "perc057", D2, "00:09", ""), Kind::Image("screenshot.png".into(), "340 КБ".into())),
                with(msg(false, "perc057", D2, "00:11", ""), Kind::Voice("0:14".into())),
                with(msg(false, "perc057", D2, "00:12", ""), Kind::Video("0:12".into())),
            ], ..Default::default() },
        Chat { name: "ганжубасик, n1kas".into(), kind: 'g', status: "3 участника, 2 в сети".into(), unread: 0, pinned: false, muted: true,
            msgs: vec![
                msg(false, "n1kas", D2, "10:30", "кто сегодня в майн?"),
                msg(true, "Вы", D2, "10:41", "я после обеда"),
                with(msg(false, "ганжубасик", D2, "11:02", ""), Kind::Call("пропущенный, 0:00".into())),
            ], ..Default::default() },
        Chat { name: "Carl-bot".into(), kind: 'b', status: "бот".into(), unread: 0, pinned: false, muted: false,
            msgs: vec![msg(false, "Carl-bot", D1, "12:00", "Привет! Напиши /help")], ..Default::default() },
        Chat { name: "Новости Rust".into(), kind: 'c', status: "12 480 подписчиков".into(), unread: 5, pinned: false, muted: false,
            msgs: vec![msg(false, "Новости Rust", D2, "08:00", "Вышел Rust 1.9x: стабилизированы новые API для async.")], ..Default::default() },
        Chat { name: "kasaen".into(), kind: 'p', status: "был(а) вчера в 23:41".into(), unread: 0, pinned: false, muted: false,
            msgs: vec![
                msg(false, "kasaen", D1, "19:51", "звони как освободишься"),
                with(msg(true, "Вы", D1, "20:10", ""), Kind::Call("исходящий, 12:31".into())),
            ], ..Default::default() },
    ];
    for (i, c) in v.iter_mut().enumerate() {
        c.id = i as i64 + 1;
        c.order = 100_000 - i as i64 + if c.pinned { 1_000_000 } else { 0 };
    }
    v
}
