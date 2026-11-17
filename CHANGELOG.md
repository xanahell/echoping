# Changelog

## 0.1.0

First public version.

- Own window (Windows, Linux, macOS) with a system tray icon; closing the window can minimize to tray.
- Terminal version (`echoping-tui`) with the same interface.
- Login by QR code or phone number, two-factor password, up to 5 accounts.
- Chats, folders (private, groups, channels, bots), favorites, archive, pinned chats, search, tabs.
- Messages: history, replies, edits, deletion, files and photos sending, read marks.
- In-client viewing of photos and video thumbnails, voice message playback (pure-Rust Opus decoder).
- Pixel emoji (Twemoji) and an emoji picker, `:shortcode:` replacement.
- Profiles of users, groups and channels; read-only channels hide the input field.
- Active sessions page; Telegram Premium / Stars overview (payment is done in the official bot).
- Ghost mode: no "read" / "listened" receipts, optional hidden online status.
- Memory saving: low-memory TDLib mode, lazy loading, memory trimming while idle.
- Resizable chat list (drag the divider), interface scale, folder label size.
- Demo mode that works without any account.

Not included: audio/video calls, in-client video playback, secret chats, stickers, reactions, polls.
