# Panda: a music bot for Evre

Panda plays music in the voice channels of your [Evre](https://evre.chat)
server: YouTube, SoundCloud and any other site [yt-dlp](https://github.com/yt-dlp/yt-dlp)
knows, or direct links to audio files and streams. It has a queue, a
now-playing panel with buttons, repeat and shuffle, and loudness
normalisation, so every song plays at about the same volume.

**You run Panda yourself**, on your own computer, a home server or a VPS:
Evre does not host bots. Panda is one small program; it only connects out
to Evre (it opens no port) and needs yt-dlp and FFmpeg next to it.

Its replies and panel speak English, French, German, Spanish or Turkish
(`PANDA_LANG`), and its commands work in all five at once.

[Français : README.fr.md](README.fr.md) · Setup guide on the website:
<https://evre.chat/en/panda/>

## What you need

- An Evre server **on a paid plan**: the Free plan allows no bots. Standard
  allows 3 bots (music at 192 kbit/s), Pro 10 and Community 25 (256 kbit/s).
- You must be allowed to manage the server (to create the bot).
- A computer that stays on while you want music: Windows 10/11 (x64),
  Linux x86_64 with glibc 2.31 or newer (Ubuntu 20.04+, Debian 11+), or
  macOS (build it yourself, see below).
- [yt-dlp](https://github.com/yt-dlp/yt-dlp) and [FFmpeg](https://ffmpeg.org)
  with `ffprobe`. Without yt-dlp only direct audio links play; without
  FFmpeg nothing plays.

## Setup

### 1. Create the bot in Evre

1. Open your server in Evre, then **Server settings → Bots → Create bot**.
2. Name it, for example `Panda`.
3. Evre shows the bot's **token** (it starts with `erabot_`) **only once**:
   copy it somewhere safe, then click **I saved it**. The token is the bot's
   password: anyone who has it can speak as your bot. If you lose it or
   leak it, **Reset token** gives a new one and the old one stops at once.

The bot joins your server right away. It needs to read and send messages
in your text channels and to connect and speak in the voice channels where
it should play; by default `@everyone` allows all of that. If a channel
denies it, give the bot a role that allows it there.

### 2. Download Panda

| System | Download |
|---|---|
| Windows x64 | [panda-windows-x86_64.zip](https://evre.chat/downloads/panda/panda-windows-x86_64.zip) |
| Linux x86_64 | [panda-linux-x86_64.tar.gz](https://evre.chat/downloads/panda/panda-linux-x86_64.tar.gz) |
| macOS | build it on a Mac (see [Building from source](#building-from-source)) |

The same files are attached to each [GitHub release](https://github.com/KaanBahaSever/evre-panda/releases),
with `SHA256SUMS`. Unpack the archive into a folder of its own.

On Windows, SmartScreen may warn about an unknown publisher the first time
(Panda is not code-signed): **More info → Run anyway**.

### 3. Install yt-dlp and FFmpeg

**Windows** (PowerShell):

```powershell
winget install yt-dlp.yt-dlp
winget install Gyan.FFmpeg
```

Open a new terminal afterwards so the programs are on the `PATH`. You can
also put `yt-dlp.exe`, `ffmpeg.exe` and `ffprobe.exe` into Panda's folder,
or point `PANDA_YTDLP` and `PANDA_FFMPEG` at them.

**Debian, Ubuntu**:

```sh
sudo apt install ffmpeg
# yt-dlp from its own releases: distribution packages are often too old for YouTube.
mkdir -p ~/.local/bin
curl -L https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp -o ~/.local/bin/yt-dlp
chmod +x ~/.local/bin/yt-dlp
```

With yt-dlp's own release binary, `PANDA_YTDLP_AUTO_UPDATE=true` lets
Panda run `yt-dlp -U` at start and once a day, which keeps YouTube working.
(It does nothing for yt-dlp from apt, pip or winget; update those the usual
way.)

**macOS**: `brew install yt-dlp ffmpeg`.

### 4. Configure

Copy `panda.env.example` to `panda.env` next to Panda and fill in the
token:

```ini
ERA_URL=https://evre.chat
ERA_BOT_TOKEN=erabot_...your token...
PANDA_LANG=en
```

Every setting can also be an environment variable, which wins over the
file. See [Settings](#settings) for the rest.

### 5. Run

```sh
panda --check                 # tests FFmpeg, yt-dlp and the voice stack, connects nowhere
panda --config panda.env
```

On Windows: `.\panda.exe --check`, then `.\panda.exe --config panda.env`.
`Panda is online as … in 1 guild(s)` in the log means it is connected.
Stop it with Ctrl+C.

### 6. Play something

Join a voice channel in Evre, then write in a text channel:

```
!play lofi hip hop
```

Panda joins your voice channel and posts a now-playing panel with buttons.
`!help` lists the commands.

## Commands

The prefix is `!` (`PANDA_PREFIX`). Every name below works whatever
`PANDA_LANG` says; names are case-insensitive, and the accented ones also
work without accents (`!arreter`, `!melanger`, `!cal`).

| Command | Other names | What it does |
|---|---|---|
| `!play <song or link>` | `!p`, `!jouer`, `!spielen`, `!reproducir`, `!çal` | Plays now, or adds to the queue. Words search YouTube; links can be YouTube, SoundCloud, any site yt-dlp knows, or an audio file or stream |
| `!skip` | `!next`, `!passer`, `!suivant`, `!überspringen`, `!saltar`, `!geç` | Next song |
| `!stop` | `!arrêter`, `!stopp`, `!parar`, `!dur` | Stops, clears the queue and leaves the channel |
| `!pause` | `!pausa`, `!duraklat` | Pauses |
| `!resume` | `!reprendre`, `!fortsetzen`, `!weiter`, `!reanudar`, `!devam` | Carries on |
| `!queue` | `!q`, `!file`, `!liste`, `!warteschlange`, `!cola`, `!sıra` | Shows the queue |
| `!np` | `!nowplaying`, `!maintenant`, `!jetzt`, `!ahora`, `!şimdi` | The song playing, with a progress bar |
| `!volume [0-200]` | `!vol`, `!lautstärke`, `!volumen`, `!ses` | Shows or sets the volume (100 is normal) |
| `!shuffle` | `!mélanger`, `!mischen`, `!mezclar`, `!karıştır` | Shuffles the queue |
| `!loop [off\|one\|all]` | `!repeat`, `!boucle`, `!wiederholen`, `!repetir`, `!tekrar` | Repeat nothing, the song or the queue; without a word it goes to the next mode |
| `!join` | `!rejoindre`, `!komm`, `!ven`, `!gel` | Comes to your voice channel |
| `!leave` | `!disconnect`, `!quitter`, `!verlassen`, `!salir`, `!çık` | Leaves; the song goes back to the front of the queue |
| `!remove <#>` | `!retirer`, `!supprimer`, `!entfernen`, `!quitar`, `!sil` | Removes a song from the queue |
| `!clear` | `!vider`, `!leeren`, `!vaciar`, `!temizle` | Empties the queue |
| `!help` | `!aide`, `!hilfe`, `!ayuda`, `!yardım` | Lists the commands |

`!loop` also takes the words of the other languages (`non/titre/tout`,
`aus/titel/alle`, `no/una/todo`, `kapalı/şarkı/hepsi`).

**The panel.** Each song gets a message with buttons: pause/resume, skip,
stop, shuffle, repeat, volume down and up, and the queue (shown only to
you).

**Who may do what.** Everyone in the server may queue songs, look at the
queue and call Panda. Controls that change playback (skip, stop, pause,
volume, repeat, shuffle, remove, clear, leave, and the panel's buttons)
need you in Panda's voice channel while Panda is in one.

Panda leaves on its own after 2 minutes with nobody listening and after 5
minutes with nothing to play (`PANDA_ALONE_MINUTES`, `PANDA_IDLE_MINUTES`).

## Settings

From `panda.env` (`--config`, or the `PANDA_CONFIG` variable) or the
environment, which wins.

| Setting | Default | Meaning |
|---|---|---|
| `ERA_URL` | (required) | Evre's address: `https://evre.chat` |
| `ERA_BOT_TOKEN` | (required) | The bot's token from Server settings → Bots |
| `PANDA_LANG` | `tr` | Language of the replies and the panel: `en`, `fr`, `de`, `es`, `tr` |
| `PANDA_PREFIX` | `!` | Command prefix |
| `PANDA_DATA_DIR` | `panda-data` | Where Panda keeps each server's volume and repeat mode |
| `PANDA_YTDLP` | `yt-dlp` | The yt-dlp program |
| `PANDA_FFMPEG` | `ffmpeg` | The FFmpeg program (`ffprobe` must be next to it) |
| `PANDA_YTDLP_AUTO_UPDATE` | `false` | Run `yt-dlp -U` at start and daily |
| `PANDA_YTDLP_COOKIES` | – | A cookies file for YouTube (see Troubleshooting) |
| `PANDA_VOLUME` | `100` | Starting volume (0–200) |
| `PANDA_LOUDNORM` | `true` | EBU R128 loudness normalisation |
| `PANDA_BITRATE_KBPS` | plan limit | Music bitrate; the server caps it at your plan's |
| `PANDA_ALONE_MINUTES` | `2` | Leave after this long without listeners |
| `PANDA_IDLE_MINUTES` | `5` | Leave after this long with nothing to play |
| `PANDA_MAX_QUEUE` | `200` | Songs in the queue at most |

`RUST_LOG=debug` (an environment variable) shows more in the log.

## Keeping it running

**Linux (systemd)**, with Panda in `/opt/panda` and its settings in
`/opt/panda/panda.env`:

```ini
# /etc/systemd/system/panda.service
[Unit]
Description=Panda music bot for Evre
After=network-online.target
Wants=network-online.target

[Service]
User=panda
WorkingDirectory=/opt/panda
ExecStart=/opt/panda/panda --config /opt/panda/panda.env
Restart=on-failure
RestartSec=10

[Install]
WantedBy=multi-user.target
```

```sh
sudo useradd --system --home /opt/panda panda && sudo chown -R panda: /opt/panda
sudo chmod 600 /opt/panda/panda.env
sudo systemctl enable --now panda
journalctl -u panda -f
```

**Windows**: run it in a terminal you leave open, or create a task in Task
Scheduler that starts `panda.exe --config panda.env` at logon, with Panda's
folder as "Start in".

## Troubleshooting

**"YouTube thinks I'm a bot" / "Sign in to confirm you're not a bot".**
YouTube blocks some addresses, datacenters and VPS providers most of all;
home connections are blocked far less. First update yt-dlp (`yt-dlp -U`,
or `PANDA_YTDLP_AUTO_UPDATE=true`). If it keeps happening, export YouTube
cookies from a browser where you are signed in (Netscape format, for
example with a "Get cookies.txt" extension) into `cookies.txt` and set
`PANDA_YTDLP_COOKIES=cookies.txt`. Use an account you do not mind; keep the
file private. SoundCloud and direct links are not affected.

**"ffmpeg is not installed" / nothing plays.** `panda --check` tells which
part is missing. Install FFmpeg (step 3) so that `ffmpeg -version` and
`ffprobe -version` work in a new terminal, or set `PANDA_FFMPEG` to the full
path of `ffmpeg` (`ffprobe` must be in the same folder).

**"yt-dlp is not installed".** Same: `yt-dlp --version` must work, or set
`PANDA_YTDLP`.

**"ERA_BOT_TOKEN is not set" or Panda stops with an authentication error.**
The token is missing, mistyped or was reset. Create a new one with **Reset
token** in Server settings → Bots.

**Panda says the server's voice limit is reached, or it cannot join.** The
plan's limit of people in voice is full, the channel is full, or the bot is
not allowed to connect or speak there (check the channel's permissions).

**It plays, but the panel's buttons say I must be in the voice channel.**
Playback controls are for the people listening: join Panda's channel.

**A link does not play: "I can't open that link (local network address)".**
Panda refuses links to this machine and to the local network on purpose,
so nobody in your server can make it reach your router or other devices.

## Building from source

You need Rust (stable, via [rustup](https://rustup.rs)) and:

- **Windows**: Visual Studio Build Tools with "Desktop development with C++".
- **Linux**: a recent `clang` (the build says which version it needs), and
  `pkg-config` with the glib development files (`libglib2.0-dev`).
- **macOS**: Xcode command-line tools.

```sh
cargo build --release --locked -p panda     # target/release/panda(.exe)
cargo test --workspace --locked
```

The first build downloads LiveKit's prebuilt libwebrtc (several hundred
MB). `python scripts/release.py --windows --linux` builds the release
archives (the Linux one cross-compiled from Windows with cargo-zigbuild, see
the script); on a Mac, `python scripts/release.py --host`.

**Layout.**

| Folder | What |
|---|---|
| `panda/` | The bot |
| `bot-sdk/` | `era-bot-sdk`, Evre's Rust bot SDK: gateway, REST, prefix commands, buttons and voice. Write your own bot with it (see `bot-sdk/examples/`) |
| `protocol/` | Evre's wire types (shared with Evre's server and apps) |
| `entitlements/` | Evre's plan definitions, which `protocol` uses |
| `vendor/` | Two LiveKit crates with small patches (`vendor/README.md`) |

**Contributing.** This repository is exported from Evre's main
(private) repository, where Panda, the SDK and the protocol are developed
together with the server. Issues and pull requests are welcome here; an
accepted change is applied there and comes back with the next export.
Comments in the code sometimes point to sections of Evre's internal design
document (`architecture.md §…`), which is not part of this repository.

## A note on YouTube

Downloading from YouTube is against YouTube's terms of service. Panda is a
tool; you decide what you play with it and are responsible for it.
SoundCloud, your own files and streams with the right to play them are the
safer sources.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT), at your option. `vendor/` holds modified copies
of LiveKit's crates under Apache-2.0 (see [NOTICE](NOTICE)).

Panda v1, the TeamSpeak music bot this one replaces, lives on at
<https://github.com/KaanBahaSever/panda> (OSL-3.0).
