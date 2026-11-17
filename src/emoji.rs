//! Пиксельные эмодзи: таблица названий и атлас спрайтов (Twemoji, CC-BY 4.0, см. NOTICE.md).
//! Файл создан скриптом tools/gen_emoji.py.

pub const CELL: u32 = 24;
pub const COLS: u32 = 16;
pub const ATLAS: &[u8] = include_bytes!("../assets/emoji_atlas.png");

/// (эмодзи, короткое имя для :имя:) в порядке спрайтов атласа.
pub const EMOJI: &[(&str, &str)] = &[
    ("😀", "grinning"),
    ("😃", "smiley"),
    ("😄", "smile"),
    ("😁", "grin"),
    ("😆", "laughing"),
    ("😅", "sweat_smile"),
    ("😂", "joy"),
    ("🤣", "rofl"),
    ("🙂", "slight_smile"),
    ("🙃", "upside_down"),
    ("😉", "wink"),
    ("😊", "blush"),
    ("😇", "innocent"),
    ("🥰", "love_face"),
    ("😍", "heart_eyes"),
    ("🤩", "star_struck"),
    ("😘", "kissing_heart"),
    ("😋", "yum"),
    ("😛", "tongue"),
    ("😜", "wink_tongue"),
    ("🤪", "zany"),
    ("🤗", "hugging"),
    ("🤔", "thinking"),
    ("🤨", "raised_brow"),
    ("😐", "neutral"),
    ("😑", "expressionless"),
    ("😶", "no_mouth"),
    ("😏", "smirk"),
    ("😒", "unamused"),
    ("🙄", "roll_eyes"),
    ("😬", "grimace"),
    ("😌", "relieved"),
    ("😔", "pensive"),
    ("😪", "sleepy"),
    ("😴", "sleeping"),
    ("😷", "mask"),
    ("🤒", "sick"),
    ("🤕", "hurt"),
    ("🤢", "nauseated"),
    ("🤮", "vomit"),
    ("🥵", "hot"),
    ("🥶", "cold"),
    ("😵", "dizzy_face"),
    ("🤯", "exploding_head"),
    ("🤠", "cowboy"),
    ("😎", "sunglasses"),
    ("🤓", "nerd"),
    ("😕", "confused"),
    ("😟", "worried"),
    ("🙁", "frown"),
    ("😮", "open_mouth"),
    ("😲", "astonished"),
    ("😳", "flushed"),
    ("🥺", "pleading"),
    ("😨", "fearful"),
    ("😰", "cold_sweat"),
    ("😢", "cry"),
    ("😭", "sob"),
    ("😱", "scream"),
    ("😖", "confounded"),
    ("😞", "disappointed"),
    ("😓", "sweat"),
    ("😩", "weary"),
    ("😫", "tired"),
    ("🥱", "yawn"),
    ("😤", "triumph"),
    ("😡", "rage"),
    ("😠", "angry"),
    ("🤬", "cursing"),
    ("😈", "smiling_imp"),
    ("👿", "imp"),
    ("💀", "skull"),
    ("💩", "poop"),
    ("🤡", "clown"),
    ("👻", "ghost"),
    ("👽", "alien"),
    ("🤖", "robot"),
    ("👍", "thumbsup"),
    ("👎", "thumbsdown"),
    ("👌", "ok_hand"),
    ("✌️", "victory"),
    ("🤞", "crossed_fingers"),
    ("🤟", "love_you"),
    ("🤘", "rock"),
    ("👈", "point_left"),
    ("👉", "point_right"),
    ("👆", "point_up"),
    ("👇", "point_down"),
    ("✋", "raised_hand"),
    ("👋", "wave"),
    ("🤝", "handshake"),
    ("👏", "clap"),
    ("🙌", "raised_hands"),
    ("🙏", "pray"),
    ("💪", "muscle"),
    ("👀", "eyes"),
    ("❤️", "heart"),
    ("🧡", "orange_heart"),
    ("💛", "yellow_heart"),
    ("💚", "green_heart"),
    ("💙", "blue_heart"),
    ("💜", "purple_heart"),
    ("🖤", "black_heart"),
    ("💔", "broken_heart"),
    ("💕", "two_hearts"),
    ("💯", "100"),
    ("💥", "boom"),
    ("💫", "dizzy"),
    ("💦", "sweat_drops"),
    ("💨", "dash"),
    ("🔥", "fire"),
    ("✨", "sparkles"),
    ("⭐", "star"),
    ("🌟", "glowing_star"),
    ("⚡", "zap"),
    ("🎉", "tada"),
    ("🎊", "confetti"),
    ("🎁", "gift"),
    ("🎂", "cake"),
    ("🍕", "pizza"),
    ("🍔", "burger"),
    ("🍟", "fries"),
    ("🍺", "beer"),
    ("🍻", "beers"),
    ("☕", "coffee"),
    ("🍷", "wine"),
    ("🎵", "music"),
    ("🎮", "game"),
    ("⚽", "soccer"),
    ("🏆", "trophy"),
    ("🚀", "rocket"),
    ("🚗", "car"),
    ("✈️", "airplane"),
    ("🏠", "house"),
    ("🌍", "earth"),
    ("🌈", "rainbow"),
    ("☀️", "sun"),
    ("🌙", "moon"),
    ("☁️", "cloud"),
    ("❄️", "snowflake"),
    ("🌹", "rose"),
    ("🌸", "blossom"),
    ("🐶", "dog"),
    ("🐱", "cat"),
    ("🐭", "mouse"),
    ("🐻", "bear"),
    ("🦊", "fox"),
    ("🐼", "panda"),
    ("🐸", "frog"),
    ("🐵", "monkey"),
    ("🙈", "see_no_evil"),
    ("🙉", "hear_no_evil"),
    ("🙊", "speak_no_evil"),
    ("🐔", "chicken"),
    ("🐧", "penguin"),
    ("🦄", "unicorn"),
    ("🐝", "bee"),
    ("🐢", "turtle"),
    ("🐍", "snake"),
    ("🐙", "octopus"),
    ("🐳", "whale"),
    ("✅", "check"),
    ("❌", "cross"),
    ("❓", "question"),
    ("❗", "exclamation"),
    ("⚠️", "warning"),
    ("🔔", "bell"),
    ("🔒", "lock"),
    ("🔑", "key"),
    ("💡", "bulb"),
    ("📌", "pin"),
    ("📎", "clip"),
    ("📷", "camera"),
    ("📱", "phone"),
    ("💻", "laptop"),
    ("📞", "call"),
    ("⏰", "alarm"),
    ("💰", "money"),
    ("🎧", "headphones"),
    ("📝", "memo"),
    ("🔍", "search"),
    ("💬", "speech"),
    ("💭", "thought"),
];

use std::{collections::HashMap, sync::OnceLock};

fn strip_vs(s: &str) -> String { s.chars().filter(|c| *c as u32 != 0xFE0F).collect() }

fn index() -> &'static HashMap<String, usize> {
    static IDX: OnceLock<HashMap<String, usize>> = OnceLock::new();
    IDX.get_or_init(|| EMOJI.iter().enumerate().map(|(i, (e, _))| (strip_vs(e), i)).collect())
}

/// Номер спрайта для эмодзи (без учёта селектора вариации U+FE0F).
pub fn index_of(sym: &str) -> Option<usize> { index().get(&strip_vs(sym)).copied() }

/// Заменяет :имя: на эмодзи («:fire:» -> огонь). Неизвестные имена остаются как есть.
pub fn expand_shortcodes(text: &str) -> String {
    if !text.contains(':') { return text.to_string(); }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(a) = rest.find(':') {
        out.push_str(&rest[..a]);
        let tail = &rest[a + 1..];
        match tail.find(':') {
            Some(b) if b > 0 && b <= 24 && tail[..b].chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '+' || c == '-') => {
                let name = &tail[..b];
                let name = match name { "+1" => "thumbsup", "-1" => "thumbsdown", n => n };
                if let Some((e, _)) = EMOJI.iter().find(|(_, n)| *n == name) { out.push_str(e); rest = &tail[b + 1..]; }
                else { out.push(':'); rest = tail; }
            }
            _ => { out.push(':'); rest = tail; }
        }
    }
    out.push_str(rest);
    out
}

/// Простая замена частых эмодзи на смайлики ASCII (для терминалов без цветных шрифтов).
pub fn to_ascii(text: &str) -> String {
    let mut s = text.to_string();
    for (e, a) in [("😀", ":D"), ("😃", ":D"), ("😄", ":D"), ("😁", ":D"), ("😆", "XD"), ("😂", "XD"), ("🤣", "XD"), ("🙂", ":)"), ("😊", ":)"), ("😉", ";)"),
        ("😍", "<3"), ("😘", ":*"), ("😢", ":'("), ("😭", ":'("), ("😡", ">:("), ("😎", "B)"), ("😮", ":O"), ("😐", ":|"), ("😛", ":P"), ("😜", ";P"),
        ("❤️", "<3"), ("❤", "<3"), ("👍", "(+1)"), ("👎", "(-1)"), ("🙏", "_/\\_"), ("🔥", "(fire)"), ("🎉", "(party)")] {
        s = s.replace(e, a);
    }
    s
}
