// Таблица настроек по образцу Telegram Desktop + свои разделы (помечены *).
// Значения хранятся в config.ini (key=value). Часть пунктов пока только
// сохраняется и начнёт действовать после подключения TDLib.
use std::{collections::HashMap, fs, path::PathBuf};

pub enum Kind { Toggle, Choice(&'static [&'static str]), Action, Info, Text }
pub struct Item { pub key: &'static str, pub label: &'static str, pub kind: Kind, pub def: &'static str }
pub struct Section { pub title: &'static str, pub custom: bool, pub items: &'static [Item] }

const fn tog(key: &'static str, label: &'static str, def: &'static str) -> Item { Item { key, label, kind: Kind::Toggle, def } }
const fn ch(key: &'static str, label: &'static str, o: &'static [&'static str], def: &'static str) -> Item { Item { key, label, kind: Kind::Choice(o), def } }
const fn act(key: &'static str, label: &'static str) -> Item { Item { key, label, kind: Kind::Action, def: "" } }
const fn info(key: &'static str, label: &'static str, def: &'static str) -> Item { Item { key, label, kind: Kind::Info, def } }
const fn txt(key: &'static str, label: &'static str, def: &'static str) -> Item { Item { key, label, kind: Kind::Text, def } }

const WHO: &[&str] = &["Все", "Мои контакты", "Никто"];
const LOCK: &[&str] = &["Выкл", "1 мин", "5 мин", "1 час", "5 часов"];
const SELFD: &[&str] = &["1 месяц", "3 месяца", "6 месяцев", "12 месяцев"];
const SIZE: &[&str] = &["1 МБ", "3 МБ", "10 МБ", "50 МБ", "100 МБ"];
const CACHE: &[&str] = &["Без лимита", "1 ГБ", "5 ГБ", "10 ГБ"];
const KEEP: &[&str] = &["1 день", "1 неделя", "1 месяц", "Всегда"];
const VOL: &[&str] = &["0%", "25%", "50%", "75%", "100%"];
const COUNT: &[&str] = &["1", "2", "3", "4", "5"];
const THEME: &[&str] = &["Тёмная", "Светлая", "Системная"];
const ACCENT: &[&str] = &["Бирюзовый", "Синий", "Зелёный", "Пурпурный", "Жёлтый"];
const BORDER: &[&str] = &["ASCII", "Unicode"];
const SEND: &[&str] = &["Enter", "Ctrl+Enter"];
const FPOS: &[&str] = &["Сверху", "Слева"];
const LANG: &[&str] = &["Русский", "English", "Українська", "Беларуская", "Deutsch", "Español"];
const REPEAT: &[&str] = &["Нет", "Все", "Один"];
const DEV: &[&str] = &["По умолчанию", "Устройство 1", "Устройство 2"];
const SPEED: &[&str] = &["0.5x", "1x", "1.5x", "2x"];
const SCALE: &[&str] = &["100%", "125%", "150%"];

pub static SECTIONS: &[Section] = &[
    Section { title: "Мой аккаунт", custom: false, items: &[
        info("name", "Имя", "Вы"), info("username", "Имя пользователя", "не задано"), info("phone", "Номер телефона", "скрыт"),
        info("premium_state", "Telegram Premium", "нет"), info("stars_state", "Звёзды", "не загружено"),
        act("sessions_open", "Активные сеансы"), act("shop_premium", "Premium: тарифы и оформление"), act("shop_stars", "Звёзды: баланс и пополнение"),
        act("logout", "Выйти из аккаунта"),
    ]},
    Section { title: "Уведомления", custom: false, items: &[
        tog("n_desktop", "Уведомления на рабочем столе", "1"), tog("n_sender", "Показывать имя отправителя", "1"),
        tog("n_preview", "Показывать текст сообщения", "1"), tog("n_sound", "Звук уведомлений", "1"),
        ch("n_vol", "Громкость", VOL, "3"), ch("n_count", "Макс. число уведомлений", COUNT, "2"),
        tog("n_priv", "Уведомления из личных чатов", "1"), tog("n_group", "Уведомления из групп", "1"),
        tog("n_chan", "Уведомления из каналов", "1"), tog("n_react", "Уведомления о реакциях", "1"),
        tog("n_counter", "Счётчик непрочитанных на значке", "1"), tog("n_muted", "Считать беззвучные чаты", "0"),
        tog("n_contact", "Сообщать, когда контакт зарегистрировался", "0"),
    ]},
    Section { title: "Данные и память", custom: false, items: &[
        tog("ad_priv", "Автозагрузка: личные чаты", "1"), tog("ad_group", "Автозагрузка: группы", "1"),
        tog("ad_chan", "Автозагрузка: каналы", "0"), tog("ad_photo", "Автозагрузка: фото", "1"),
        tog("ad_video", "Автозагрузка: видео", "0"), tog("ad_file", "Автозагрузка: файлы", "0"),
        ch("ad_size", "Лимит автозагрузки файлов", SIZE, "2"), tog("autoplay_gif", "Автовоспроизведение GIF", "1"),
        tog("autoplay_video", "Автовоспроизведение видео", "0"), txt("dl_dir", "Папка загрузок", ""),
        tog("dl_ask", "Спрашивать, куда сохранять", "0"), ch("cache_max", "Макс. размер кэша", CACHE, "1"),
        ch("cache_keep", "Хранить кэш", KEEP, "2"), info("cache_size", "Размер кэша", "0 МБ (демо)"),
        act("cache_clear", "Очистить кэш"), tog("proxy", "Использовать прокси (SOCKS5)", "0"),
    ]},
    Section { title: "Конфиденциальность", custom: false, items: &[
        info("blocked", "Заблокированные пользователи", "нет"), ch("p_phone", "Номер телефона", WHO, "2"),
        ch("p_seen", "Был(а) в сети", WHO, "0"), ch("p_photo", "Фото профиля", WHO, "0"),
        ch("p_bio", "О себе", WHO, "0"), ch("p_birth", "День рождения", WHO, "1"),
        ch("p_fwd", "Пересылка сообщений", WHO, "0"), ch("p_calls", "Звонки", WHO, "0"),
        ch("p_voice", "Голосовые сообщения", WHO, "0"), ch("p_msgs", "Сообщения", WHO, "0"),
        ch("p_invite", "Приглашения в группы", WHO, "0"), tog("passcode", "Код-пароль", "0"),
        ch("autolock", "Автоблокировка", LOCK, "0"), act("cloudpass", "Облачный пароль (двухфакторная защита)"),
        act("sessions_open", "Активные сеансы: устройства и их настройки"), ch("selfdestruct", "Удалить аккаунт при неактивности", SELFD, "3"),
        tog("sync_contacts", "Синхронизировать контакты", "0"), tog("suggest_contacts", "Предлагать частых контактов", "1"),
        tog("arch_unknown", "Архивировать новые чаты от незнакомцев", "0"),
    ]},
    Section { title: "Настройки чатов", custom: false, items: &[
        ch("theme", "Тема", THEME, "0"), ch("scale", "Масштаб интерфейса", SCALE, "0"),
        ch("send_mode", "Отправка сообщения", SEND, "0"), tog("emoji_suggest", "Предлагать эмодзи", "1"),
        tog("emoji_replace", "Заменять эмодзи на ASCII", "0"), tog("link_preview", "Превью ссылок", "1"),
        tog("time24", "24-часовой формат времени", "1"), tog("ticks", "Показывать галочки доставки", "1"),
        tog("compact", "Компактные сообщения", "0"), tog("spellcheck", "Проверка орфографии", "0"),
    ]},
    Section { title: "Папки с чатами", custom: false, items: &[
        act("folder_new", "Создать папку"), ch("folder_pos", "Расположение папок", FPOS, "0"),
        tog("folder_counter", "Счётчики на папках", "1"), info("f1", "Личные", "по типу чата"),
        info("f2", "Группы", "по типу чата"), info("f3", "Каналы", "по типу чата"), info("f4", "Боты", "по типу чата"),
    ]},
    Section { title: "Дополнительно", custom: false, items: &[
        tog("autostart", "Запускать вместе с системой", "0"), tog("start_min", "Запускать свёрнутым", "0"),
        tog("tray", "Значок в трее", "1"), tog("close_tray", "Закрытие в трей", "0"),
        tog("auto_update", "Автоматически обновлять", "1"), info("version", "Версия", "echoping 0.1.0"),
        act("export", "Экспорт данных Telegram"), tog("exp_bigphoto", "Экспериментально: фото больше 1280px", "0"),
    ]},
    Section { title: "Динамики и камера", custom: false, items: &[
        ch("out_dev", "Устройство вывода", DEV, "0"), ch("out_vol", "Громкость вывода", VOL, "4"),
        ch("in_dev", "Микрофон", DEV, "0"), act("mic_test", "Проверить микрофон"),
        ch("cam_dev", "Камера", DEV, "0"), tog("noise", "Шумоподавление", "1"), tog("echo", "Эхоподавление", "1"),
    ]},
    Section { title: "Батарея и анимации", custom: false, items: &[
        tog("save_power", "Экономия энергии при низком заряде", "0"), tog("anim_ui", "Анимации интерфейса", "1"),
        tog("anim_stickers", "Анимированные стикеры", "1"), tog("anim_emoji", "Анимированные эмодзи", "1"),
        tog("anim_effects", "Эффекты в чатах и реакции", "1"), tog("anim_autoplay", "Автовоспроизведение видео и GIF", "1"),
        ch("play_speed", "Скорость воспроизведения", SPEED, "1"),
    ]},
    Section { title: "Язык", custom: false, items: &[
        ch("lang", "Язык интерфейса", LANG, "0"), tog("translate", "Показывать кнопку «Перевести»", "1"),
        ch("translate_to", "Переводить на", LANG, "0"),
    ]},
    Section { title: "Telegram Premium", custom: false, items: &[info("premium_state2", "Статус", "нет"), act("shop_premium", "Тарифы и оформление")] },
    Section { title: "Мои звёзды", custom: false, items: &[info("stars_state2", "Баланс", "не загружено"), act("shop_stars", "Баланс и пополнение")] },
    Section { title: "Telegram для бизнеса", custom: false, items: &[info("business", "Статус", "недоступно в прототипе")] },
    Section { title: "Отправить подарок", custom: false, items: &[info("gift", "Статус", "недоступно в прототипе")] },
    Section { title: "Невидимка *", custom: true, items: &[
        tog("ghost", "Режим невидимки (Ctrl+G)", "0"), tog("ghost_read", "Не отмечать сообщения прочитанными", "1"),
        tog("ghost_listen", "Не отмечать голосовые/видео прослушанными", "1"), tog("ghost_typing", "Не отправлять «печатает...»", "1"),
        tog("ghost_offline", "Скрывать статус «в сети»", "0"),
    ]},
    Section { title: "Музыкальный плеер *", custom: true, items: &[
        txt("music_dir", "Папка с музыкой", ""), tog("pl_autonext", "Следующий трек автоматически", "1"),
        tog("pl_shuffle", "Перемешивать", "0"), ch("pl_repeat", "Повтор", REPEAT, "0"),
        ch("pl_vol", "Громкость", VOL, "3"), act("pl_rescan", "Пересканировать папку"),
    ]},
    Section { title: "Память *", custom: true, items: &[
        tog("lowmem", "Экономия ОЗУ (без базы сообщений, нужен перезапуск)", "1"), tog("trim", "Освобождать память в простое", "1"),
        info("ram_now", "Занято сейчас (рабочий набор)", "..."),
    ]},
    Section { title: "Внешний вид echoping *", custom: true, items: &[
        tog("mouse", "Управление мышью", "1"), tog("avatars", "Аватарки", "1"),
        ch("accent", "Акцентный цвет", ACCENT, "0"), ch("border", "Рамки", BORDER, "0"),
    ]},
];

pub fn find(key: &str) -> Option<&'static Item> {
    SECTIONS.iter().flat_map(|s| s.items.iter()).find(|i| i.key == key)
}

pub fn config_dir() -> PathBuf {
    if let Ok(a) = std::env::var("APPDATA") { return PathBuf::from(a).join("echoping"); }
    if let Ok(h) = std::env::var("HOME") { return PathBuf::from(h).join(".config").join("echoping"); }
    PathBuf::from(".")
}

fn home() -> PathBuf {
    std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).map(PathBuf::from).unwrap_or_default()
}

pub struct Settings { vals: HashMap<String, String>, path: PathBuf, rt: HashMap<String, String> }

impl Settings {
    pub fn load() -> Self {
        let path = config_dir().join("config.ini");
        let mut vals = HashMap::new();
        if let Ok(s) = fs::read_to_string(&path) {
            for l in s.lines() { if let Some((k, v)) = l.split_once('=') { vals.insert(k.trim().into(), v.trim().into()); } }
        }
        Settings { vals, path, rt: HashMap::new() }
    }
    pub fn save(&self) {
        let _ = fs::create_dir_all(self.path.parent().unwrap_or(&PathBuf::from(".")));
        let mut keys: Vec<_> = self.vals.iter().collect();
        keys.sort();
        let s: String = keys.iter().map(|(k, v)| format!("{k}={v}\n")).collect();
        let _ = fs::write(&self.path, s);
    }
    pub fn get(&self, key: &str) -> String {
        self.vals.get(key).cloned().unwrap_or_else(|| find(key).map(|i| i.def.to_string()).unwrap_or_default())
    }
    /// Временные значения (данные аккаунта, расход памяти): не сохраняются в файл.
    pub fn set_rt(&mut self, key: &str, v: String) { self.rt.insert(key.into(), v); }
    pub fn shown_rt(&self, key: &str) -> String { self.rt.get(key).cloned().unwrap_or_default() }
    pub fn on(&self, key: &str) -> bool { self.get(key) == "1" }
    pub fn idx(&self, key: &str) -> usize { self.get(key).parse().unwrap_or(0) }
    pub fn set(&mut self, key: &str, v: String) { self.vals.insert(key.into(), v.replace('\n', " ")); self.save(); }
    pub fn cycle(&mut self, key: &str, dir: i32) {
        if let Some(Item { kind: Kind::Choice(o), .. }) = find(key) {
            let n = o.len() as i32;
            let i = (self.idx(key) as i32 + dir).rem_euclid(n);
            self.set(key, i.to_string());
        }
    }
    pub fn music_dir(&self) -> PathBuf {
        let d = self.get("music_dir");
        if d.is_empty() { home().join("Music") } else { PathBuf::from(d) }
    }
    pub fn dl_dir(&self) -> PathBuf {
        let d = self.get("dl_dir");
        if d.is_empty() { home().join("Downloads") } else { PathBuf::from(d) }
    }
    /// Человекочитаемое значение пункта для отображения.
    pub fn shown(&self, it: &Item) -> String {
        match &it.kind {
            Kind::Toggle => if self.on(it.key) { "[x]".into() } else { "[ ]".into() },
            Kind::Choice(o) => format!("< {} >", o.get(self.idx(it.key)).copied().unwrap_or("?")),
            Kind::Text => {
                let v = self.get(it.key);
                if v.is_empty() {
                    match it.key { "music_dir" => self.music_dir().display().to_string(), "dl_dir" => self.dl_dir().display().to_string(), _ => "не задано".into() }
                } else { v }
            }
            Kind::Info => self.rt.get(it.key).cloned().unwrap_or_else(|| it.def.to_string()),
            Kind::Action => "[ Enter ]".into(),
        }
    }
}
