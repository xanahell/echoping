//! Воспроизведение голосовых сообщений Telegram (Ogg/Opus) прямо в клиенте.
//! Декодер Opus написан на чистом Rust (крейт opus-decoder), без внешних библиотек.
use std::{
    io::Cursor,
    path::PathBuf,
    sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex},
    thread,
    time::Duration,
};
use opus_decoder::OpusDecoder;
use rodio::{buffer::SamplesBuffer, OutputStream, Sink};

/// Декодирует Ogg/Opus в PCM (i16, 48 кГц). Возвращает отсчёты и число каналов.
pub fn decode_ogg_opus(data: Vec<u8>) -> Result<(Vec<i16>, u16), String> {
    let mut rd = ogg::PacketReader::new(Cursor::new(data));
    let mut dec: Option<OpusDecoder> = None;
    let (mut ch, mut skip) = (1usize, 0usize);
    let mut pcm: Vec<i16> = vec![];
    let mut n = 0;
    while let Some(p) = rd.read_packet().map_err(|e| format!("ogg: {e}"))? {
        n += 1;
        if n == 1 {
            if p.data.len() < 19 || &p.data[..8] != b"OpusHead" { return Err("это не Opus-файл".into()); }
            ch = p.data[9] as usize;
            if ch == 0 || ch > 2 { return Err("неподдерживаемое число каналов".into()); }
            skip = u16::from_le_bytes([p.data[10], p.data[11]]) as usize;
            dec = Some(OpusDecoder::new(48000, ch).map_err(|e| format!("opus: {e:?}"))?);
            continue;
        }
        if n == 2 { continue; } // OpusTags
        let d = dec.as_mut().ok_or("нет заголовка Opus")?;
        let mut buf = vec![0i16; 5760 * ch];
        let got = d.decode(&p.data, &mut buf, false).map_err(|e| format!("opus: {e:?}"))?;
        pcm.extend_from_slice(&buf[..got * ch]);
    }
    let skip = (skip * ch).min(pcm.len());
    pcm.drain(..skip);
    if pcm.is_empty() { return Err("пустая запись".into()); }
    Ok((pcm, ch as u16))
}

/// Проигрыватель одного голосового: декодирование и звук работают в своём потоке.
pub struct VoicePlayer {
    playing: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
}

impl VoicePlayer {
    pub fn new() -> Self { VoicePlayer { playing: Arc::new(AtomicBool::new(false)), stop: Arc::new(AtomicBool::new(false)), error: Arc::new(Mutex::new(None)) } }
    pub fn is_playing(&self) -> bool { self.playing.load(Ordering::Relaxed) }
    pub fn stop(&mut self) { self.stop.store(true, Ordering::Relaxed); }
    pub fn take_error(&self) -> Option<String> { self.error.lock().ok().and_then(|mut e| e.take()) }
    pub fn play(&mut self, path: PathBuf) {
        self.stop.store(true, Ordering::Relaxed); // остановить предыдущее
        let (playing, stop, error) = (Arc::new(AtomicBool::new(true)), Arc::new(AtomicBool::new(false)), self.error.clone());
        self.playing = playing.clone(); self.stop = stop.clone();
        thread::spawn(move || {
            let res = (|| -> Result<(), String> {
                let data = std::fs::read(&path).map_err(|e| format!("{e}"))?;
                let (pcm, ch) = decode_ogg_opus(data)?;
                if stop.load(Ordering::Relaxed) { return Ok(()); }
                let (_stream, handle) = OutputStream::try_default().map_err(|e| format!("нет аудиоустройства: {e}"))?;
                let sink = Sink::try_new(&handle).map_err(|e| format!("{e}"))?;
                sink.append(SamplesBuffer::new(ch, 48000, pcm));
                while !sink.empty() && !stop.load(Ordering::Relaxed) { thread::sleep(Duration::from_millis(40)); }
                sink.stop();
                Ok(())
            })();
            if let Err(e) = res { if let Ok(mut g) = error.lock() { *g = Some(e); } }
            playing.store(false, Ordering::Relaxed);
        });
    }
}
