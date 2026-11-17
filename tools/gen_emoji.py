import urllib.request, io, sys
from concurrent.futures import ThreadPoolExecutor
from PIL import Image
L = """😀 grinning|😃 smiley|😄 smile|😁 grin|😆 laughing|😅 sweat_smile|😂 joy|🤣 rofl|🙂 slight_smile|🙃 upside_down|😉 wink|😊 blush|😇 innocent|🥰 love_face|😍 heart_eyes|🤩 star_struck|😘 kissing_heart|😋 yum|😛 tongue|😜 wink_tongue|🤪 zany|🤗 hugging|🤔 thinking|🤨 raised_brow|😐 neutral|😑 expressionless|😶 no_mouth|😏 smirk|😒 unamused|🙄 roll_eyes|😬 grimace|😌 relieved|😔 pensive|😪 sleepy|😴 sleeping|😷 mask|🤒 sick|🤕 hurt|🤢 nauseated|🤮 vomit|🥵 hot|🥶 cold|😵 dizzy_face|🤯 exploding_head|🤠 cowboy|😎 sunglasses|🤓 nerd|😕 confused|😟 worried|🙁 frown|😮 open_mouth|😲 astonished|😳 flushed|🥺 pleading|😨 fearful|😰 cold_sweat|😢 cry|😭 sob|😱 scream|😖 confounded|😞 disappointed|😓 sweat|😩 weary|😫 tired|🥱 yawn|😤 triumph|😡 rage|😠 angry|🤬 cursing|😈 smiling_imp|👿 imp|💀 skull|💩 poop|🤡 clown|👻 ghost|👽 alien|🤖 robot|👍 thumbsup|👎 thumbsdown|👌 ok_hand|✌️ victory|🤞 crossed_fingers|🤟 love_you|🤘 rock|👈 point_left|👉 point_right|👆 point_up|👇 point_down|✋ raised_hand|👋 wave|🤝 handshake|👏 clap|🙌 raised_hands|🙏 pray|💪 muscle|👀 eyes|❤️ heart|🧡 orange_heart|💛 yellow_heart|💚 green_heart|💙 blue_heart|💜 purple_heart|🖤 black_heart|💔 broken_heart|💕 two_hearts|💯 100|💥 boom|💫 dizzy|💦 sweat_drops|💨 dash|🔥 fire|✨ sparkles|⭐ star|🌟 glowing_star|⚡ zap|🎉 tada|🎊 confetti|🎁 gift|🎂 cake|🍕 pizza|🍔 burger|🍟 fries|🍺 beer|🍻 beers|☕ coffee|🍷 wine|🎵 music|🎮 game|⚽ soccer|🏆 trophy|🚀 rocket|🚗 car|✈️ airplane|🏠 house|🌍 earth|🌈 rainbow|☀️ sun|🌙 moon|☁️ cloud|❄️ snowflake|🌹 rose|🌸 blossom|🐶 dog|🐱 cat|🐭 mouse|🐻 bear|🦊 fox|🐼 panda|🐸 frog|🐵 monkey|🙈 see_no_evil|🙉 hear_no_evil|🙊 speak_no_evil|🐔 chicken|🐧 penguin|🦄 unicorn|🐝 bee|🐢 turtle|🐍 snake|🐙 octopus|🐳 whale|✅ check|❌ cross|❓ question|❗ exclamation|⚠️ warning|🔔 bell|🔒 lock|🔑 key|💡 bulb|📌 pin|📎 clip|📷 camera|📱 phone|💻 laptop|📞 call|⏰ alarm|💰 money|🎧 headphones|📝 memo|🔍 search|💬 speech|💭 thought"""
items=[x.split(" ") for x in L.split("|")]
def hexname(e, strip):
    cps=[c for c in e if not (strip and ord(c)==0xfe0f)]
    return "-".join(f"{ord(c):x}" for c in cps)
def fetch(it):
    e,name=it
    for strip in (True,False):
        u=f"https://raw.githubusercontent.com/jdecked/twemoji/main/assets/72x72/{hexname(e,strip)}.png"
        try:
            data=urllib.request.urlopen(u,timeout=20).read()
            return (e,name,Image.open(io.BytesIO(data)).convert("RGBA").resize((24,24),Image.LANCZOS))
        except Exception: pass
    return (e,name,None)
with ThreadPoolExecutor(16) as ex: res=list(ex.map(fetch,items))
ok=[r for r in res if r[2] is not None]; bad=[r[1] for r in res if r[2] is None]
print("ok",len(ok),"failed",bad)
COLS=16; rows=(len(ok)+COLS-1)//COLS
atlas=Image.new("RGBA",(COLS*24,rows*24),(0,0,0,0))
for i,(e,n,im) in enumerate(ok): atlas.paste(im,((i%COLS)*24,(i//COLS)*24))
atlas.save("/home/claude/echoping/assets/emoji_atlas.png",optimize=True)
rs="//! Пиксельные эмодзи: таблица названий и атлас спрайтов (Twemoji, CC-BY 4.0, см. NOTICE.md).\n//! Файл создан скриптом tools/gen_emoji.py.\n\npub const CELL: u32 = 24;\npub const COLS: u32 = %d;\npub const ATLAS: &[u8] = include_bytes!(\"../assets/emoji_atlas.png\");\n\n/// (эмодзи, короткое имя для :имя:) в порядке спрайтов атласа.\npub const EMOJI: &[(&str, &str)] = &[\n"%COLS
for e,n,_ in ok: rs+='    ("%s", "%s"),\n'%(e,n)
rs+="];\n"
open("/tmp/emo/emoji_list.rs","w").write(rs)
