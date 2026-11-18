// Встроенный музыкальный плеер: mp3 / flac / wav / ogg (vorbis) через rodio.
// Аудиоустройство открывается лениво, при первом воспроизведении (экономия ОЗУ).
use std::{fs::{self, File}, io::BufReader, path::{Path, PathBuf}, time::{Duration, Instant}};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};

pub struct Track { pub path: PathBuf, pub title: String, pub dur: Option<Duration> }

pub struct Player {
    pub tracks: Vec<Track>,
    pub cur: Option<usize>,
    pub playing: bool,
    pub volume: u8,
    pub shuffle: bool,
    pub repeat: u8, // 0 нет, 1 все, 2 один
    pub cursor: usize,
    pub err: String,
    pub scanned: bool,
    pub dir: PathBuf,
    stream: Option<(OutputStream, OutputStreamHandle)>,
    sink: Option<Sink>,
    base: Duration,
    since: Option<Instant>,
    rng: u64,
}

fn walk(dir: &Path, depth: u8, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() { if depth > 0 { walk(&p, depth - 1, out); } }
        else if let Some(ext) = p.extension().and_then(|x| x.to_str()) {
            if matches!(ext.to_lowercase().as_str(), "mp3" | "flac" | "wav" | "ogg") { out.push(p); }
        }
    }
}

impl Player {
    pub fn new() -> Self {
        Player { tracks: vec![], cur: None, playing: false, volume: 75, shuffle: false, repeat: 0, cursor: 0,
            err: String::new(), scanned: false, dir: PathBuf::new(), stream: None, sink: None,
            base: Duration::ZERO, since: None,
            rng: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1) | 1 }
    }
    pub fn scan(&mut self, dir: &Path) {
        self.stop();
        self.dir = dir.to_path_buf();
        let mut v = vec![];
        walk(dir, 2, &mut v);
        v.sort();
        self.tracks = v.into_iter().map(|p| Track {
            title: p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(), path: p, dur: None }).collect();
        self.cur = None; self.cursor = 0; self.scanned = true; self.err.clear();
    }
    fn ensure_stream(&mut self) -> bool {
        if self.stream.is_none() {
            match OutputStream::try_default() {
                Ok(s) => self.stream = Some(s),
                Err(e) => { self.err = format!("нет аудиоустройства: {e}"); return false; }
            }
        }
        true
    }
    pub fn stop(&mut self) {
        if let Some(s) = self.sink.take() { s.stop(); }
        self.playing = false; self.cur = None; self.base = Duration::ZERO; self.since = None;
    }
    pub fn play_index(&mut self, i: usize, from: Duration) {
        if i >= self.tracks.len() || !self.ensure_stream() { return; }
        let path = self.tracks[i].path.clone();
        let file = match File::open(&path) { Ok(f) => f, Err(e) => { self.err = format!("{e}"); return; } };
        let dec = match Decoder::new(BufReader::new(file)) { Ok(d) => d, Err(e) => { self.err = format!("не удалось декодировать: {e}"); return; } };
        if let Some(d) = dec.total_duration() { self.tracks[i].dur = Some(d); }
        let handle = &self.stream.as_ref().unwrap().1;
        let sink = match Sink::try_new(handle) { Ok(s) => s, Err(e) => { self.err = format!("{e}"); return; } };
        sink.set_volume(self.volume as f32 / 100.0);
        if from > Duration::ZERO { sink.append(dec.skip_duration(from)); } else { sink.append(dec); }
        if let Some(old) = self.sink.replace(sink) { old.stop(); }
        self.cur = Some(i); self.cursor = i; self.playing = true;
        self.base = from; self.since = Some(Instant::now()); self.err.clear();
    }
    pub fn elapsed(&self) -> Duration {
        let mut e = self.base + self.since.map(|s| if self.playing { s.elapsed() } else { Duration::ZERO }).unwrap_or_default();
        if let Some(d) = self.cur.and_then(|c| self.tracks[c].dur) { if e > d { e = d; } }
        e
    }
    pub fn toggle(&mut self) {
        match (self.cur, &self.sink) {
            (Some(_), Some(s)) => {
                if self.playing { s.pause(); self.base = self.elapsed(); self.since = None; self.playing = false; }
                else { s.play(); self.since = Some(Instant::now()); self.playing = true; }
            }
            _ => { if !self.tracks.is_empty() { let i = self.cursor.min(self.tracks.len() - 1); self.play_index(i, Duration::ZERO); } }
        }
    }
    fn rand(&mut self) -> usize {
        self.rng ^= self.rng << 13; self.rng ^= self.rng >> 7; self.rng ^= self.rng << 17;
        self.rng as usize
    }
    fn next_index(&mut self) -> Option<usize> {
        let n = self.tracks.len();
        if n == 0 { return None; }
        let c = self.cur.unwrap_or(0);
        if self.shuffle && n > 1 { let mut i = self.rand() % n; if i == c { i = (i + 1) % n; } return Some(i); }
        if c + 1 < n { Some(c + 1) } else if self.repeat == 1 { Some(0) } else { None }
    }
    pub fn next(&mut self) { if let Some(i) = self.next_index() { self.play_index(i, Duration::ZERO); } else { self.stop(); } }
    pub fn prev(&mut self) {
        if self.elapsed() > Duration::from_secs(3) { if let Some(c) = self.cur { self.play_index(c, Duration::ZERO); } return; }
        let n = self.tracks.len();
        if n == 0 { return; }
        let c = self.cur.unwrap_or(0);
        self.play_index(if c == 0 { n - 1 } else { c - 1 }, Duration::ZERO);
    }
    pub fn seek_to(&mut self, pos: Duration) {
        if let Some(c) = self.cur { let was = self.playing; self.play_index(c, pos); if !was { self.toggle(); } }
    }
    pub fn seek_rel(&mut self, secs: i64) {
        let e = self.elapsed().as_secs() as i64;
        self.seek_to(Duration::from_secs((e + secs).max(0) as u64));
    }
    pub fn seek_frac(&mut self, f: f32) {
        if let Some(d) = self.cur.and_then(|c| self.tracks[c].dur) { self.seek_to(d.mul_f32(f.clamp(0.0, 0.999))); }
    }
    pub fn set_volume(&mut self, v: i32) {
        self.volume = v.clamp(0, 100) as u8;
        if let Some(s) = &self.sink { s.set_volume(self.volume as f32 / 100.0); }
    }
    /// Вызывать периодически: переход на следующий трек по окончании.
    pub fn tick(&mut self, autonext: bool) {
        if self.playing && self.sink.as_ref().map(|s| s.empty()).unwrap_or(false) {
            if self.repeat == 2 { if let Some(c) = self.cur { self.play_index(c, Duration::ZERO); } }
            else if autonext { self.next(); } else { self.stop(); }
        }
    }
    pub fn title(&self) -> String {
        self.cur.map(|c| self.tracks[c].title.clone()).unwrap_or_default()
    }
}

pub fn fmt(d: Duration) -> String { format!("{:02}:{:02}", d.as_secs() / 60, d.as_secs() % 60) }
