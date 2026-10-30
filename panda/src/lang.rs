//! What Panda says, in each of Evre's five languages (`PANDA_LANG`: `tr`,
//! `en`, `fr`, `de`, `es`; Turkish by default).
//!
//! Every text is a template: `{name}` is filled in by [`fill`] (`{p}` is
//! the command prefix). The commands themselves work in every language at
//! once (`bot::ALIASES`); only the replies, the panel and its buttons follow
//! the setting. Tests check that every language has the same placeholders
//! as the Turkish text and that every command a text mentions exists.

use std::fmt::Write as _;

pub use era_bot_sdk::protocol::Locale;

/// Declares [`Strings`] and its field list (for the tests).
macro_rules! strings {
    ($($(#[$doc:meta])* $name:ident,)*) => {
        /// One language's texts.
        #[derive(Debug)]
        pub struct Strings {
            $($(#[$doc])* pub $name: &'static str,)*
        }

        impl Strings {
            /// Every text with its field name.
            #[must_use]
            pub fn fields(&self) -> Vec<(&'static str, &'static str)> {
                vec![$((stringify!($name), self.$name)),*]
            }
        }
    };
}

strings! {
    /// A title that is empty after cleaning.
    untitled,
    /// A live stream's length.
    live,
    /// The panel's first words while the song plays.
    now_label,
    /// The panel's first words once the song is over.
    played_label,
    /// `{name}`.
    requested_by,
    /// `{n}`, `{title}`, `{length}`.
    queued,
    /// A live stream's position: `{time}`.
    listening,
    /// `{name}`, `{volume}`.
    now_details,
    /// Appended to `!np` while paused.
    now_paused,
    /// Appended to `!np` when repeating the song.
    now_loop_one,
    /// Appended to `!np` when repeating the queue.
    now_loop_all,
    /// `{p}`.
    queue_empty,
    /// Nothing waits after the current song.
    queue_no_more,
    /// `{songs}`, `{time}`.
    queue_header,
    /// `{songs}`.
    queue_more,
    /// The queue's last line when repeating the song.
    queue_loop_one,
    /// The queue's last line when repeating the queue.
    queue_loop_all,
    /// `{n}` (one song).
    song_one,
    /// `{n}` (other counts).
    song_many,
    /// `{n}` percent.
    percent,
    /// A playback control from outside Panda's voice channel.
    not_in_channel,
    /// Repeat set to off.
    loop_off,
    /// Repeat set to the song.
    loop_one,
    /// Repeat set to the queue.
    loop_all,
    /// `!help`: `{p}`.
    help,
    /// YouTube's "confirm you're not a bot".
    err_bot_check,
    /// Age-restricted.
    err_age,
    /// Private or removed.
    err_unavailable,
    /// The search found nothing.
    err_not_found,
    /// yt-dlp is missing.
    err_missing_ytdlp,
    /// ffmpeg is missing.
    err_missing_ffmpeg,
    /// The lookup took too long.
    err_timeout,
    /// A link to a local address.
    err_local,
    /// A link nothing knows.
    err_unsupported,
    /// Anything else (the details stay in the log).
    err_other,
    /// `!play` without words: `{p}`.
    play_what,
    /// `!play` from outside any voice channel.
    voice_first_call,
    /// `!join` from outside any voice channel.
    voice_first,
    /// `!skip`.
    skipped,
    /// Nothing plays.
    nothing_playing,
    /// `!stop`.
    stopped,
    /// `!stop` with nothing playing.
    stop_nothing,
    /// `!pause`: `{p}`.
    paused,
    /// `!pause` with nothing playing.
    pause_nothing,
    /// `!resume`.
    resumed,
    /// `!resume` with nothing paused.
    resume_nothing,
    /// `!volume`: `{volume}`.
    volume_is,
    /// `!volume N`: `{volume}`.
    volume_set,
    /// A volume out of range: `{p}`.
    volume_range,
    /// `!shuffle` with fewer than two songs.
    shuffle_few,
    /// `!shuffle`: `{songs}`.
    shuffled,
    /// `!loop` with a word it does not know: `{p}`.
    loop_usage,
    /// `!join`.
    coming,
    /// `!join` in Panda's own channel.
    already_here,
    /// `!leave`: `{p}`.
    left,
    /// `!leave` outside voice.
    not_in_voice,
    /// `!remove` without a number: `{p}`.
    remove_usage,
    /// `!remove N`: `{title}`.
    removed,
    /// `!remove N` past the end: `{n}`.
    no_such,
    /// `!clear` on an empty queue.
    clear_empty,
    /// `!clear`: `{songs}`.
    cleared,
    /// A button Panda does not know.
    unknown_button,
    /// No permission to join.
    join_forbidden,
    /// The plan's voice limit.
    join_plan_full,
    /// A full channel, or not a voice channel.
    join_channel_full,
    /// No permission to speak.
    join_no_speak,
    /// Any other join failure.
    join_failed,
    /// The queue is full: `{songs}`.
    queue_full,
    /// A song failed twice: `{title}`, `{reason}`.
    track_failed,
    /// The voice connection dropped: `{p}`.
    voice_lost,
    /// Leaving, nobody listens.
    alone_leaving,
    /// Leaving, nothing to play: `{p}`.
    idle_leaving,
    /// The panel's state line: paused.
    panel_paused,
    /// The panel's state line: repeating the song.
    panel_loop_one,
    /// The panel's state line: repeating the queue.
    panel_loop_all,
    /// The panel's state line: nothing queued.
    panel_queue_empty,
    /// The panel's state line: `{songs}` queued.
    panel_queued,
    /// ⏯ while playing.
    button_pause,
    /// ⏯ while paused.
    button_resume,
    /// ⏭.
    button_skip,
    /// ⏹.
    button_stop,
    /// 🔀.
    button_shuffle,
    /// 🔁 off.
    button_loop_off,
    /// 🔂 the song.
    button_loop_one,
    /// 🔁 the queue.
    button_loop_all,
    /// 🔉.
    button_volume_down,
    /// 🔊.
    button_volume_up,
    /// 📜.
    button_queue,
}

/// Turkish, the original (casual, like Panda v1).
pub static TR: Strings = Strings {
    untitled: "Adsız şarkı",
    live: "canlı",
    now_label: "🐼 Şimdi çalıyor:",
    played_label: "🎵 Çaldı:",
    requested_by: "isteyen: {name}",
    queued: "Sıraya ekledim (#{n}): **{title}** ({length})",
    listening: "{time} dinleniyor",
    now_details: "İsteyen: {name} · Ses: {volume}",
    now_paused: " · ⏸ duraklatıldı",
    now_loop_one: " · 🔂 bu şarkı tekrar",
    now_loop_all: " · 🔁 sıra tekrar",
    queue_empty: "Sıra boş. `{p}çal <şarkı adı ya da link>` ile bir şey ekle 🎵",
    queue_no_more: "Sırada başka şarkı yok.",
    queue_header: "Sırada {songs} var ({time}):",
    queue_more: "…ve {songs} daha",
    queue_loop_one: "🔂 Bu şarkı tekrar ediyor.",
    queue_loop_all: "🔁 Sıra tekrar ediyor.",
    song_one: "{n} şarkı",
    song_many: "{n} şarkı",
    percent: "%{n}",
    not_in_channel: "Bunun için benimle aynı ses kanalında olmalısın 🙂",
    loop_off: "Tekrar kapalı.",
    loop_one: "🔂 Bu şarkı tekrar edecek.",
    loop_all: "🔁 Tüm sıra tekrar edecek.",
    help: "🐼 **Panda komutları**\n\
           `{p}çal <şarkı adı ya da link>` (`{p}play`, `{p}p`): çalar ya da sıraya ekler (YouTube, SoundCloud, dosya linki)\n\
           `{p}geç` (`{p}skip`): sonraki şarkı\n\
           `{p}dur` (`{p}stop`): durdurur, sırayı temizler ve çıkar\n\
           `{p}duraklat` (`{p}pause`) · `{p}devam` (`{p}resume`)\n\
           `{p}sıra` (`{p}queue`, `{p}q`): sırayı gösterir\n\
           `{p}şimdi` (`{p}np`): şu an çalan\n\
           `{p}ses <0-200>` (`{p}volume`): ses seviyesi\n\
           `{p}karıştır` (`{p}shuffle`): sırayı karıştırır\n\
           `{p}tekrar [kapalı|şarkı|hepsi]` (`{p}loop [off|one|all]`)\n\
           `{p}sil <#>` (`{p}remove`) · `{p}temizle` (`{p}clear`)\n\
           `{p}gel` (`{p}join`): kanalına gelir · `{p}çık` (`{p}leave`): kanaldan çıkar\n\
           `{p}yardım` (`{p}help`): bu liste",
    err_bot_check: "YouTube beni bot sandı 🙈 Biraz sonra tekrar dene (ya da yöneticiden çerez dosyası eklemesini iste).",
    err_age: "Bu video yaş sınırlı, açamıyorum.",
    err_unavailable: "Bu video kullanılamıyor (gizli ya da kaldırılmış).",
    err_not_found: "Bir şey bulamadım 🤔",
    err_missing_ytdlp: "yt-dlp kurulu değil, yöneticiye haber ver.",
    err_missing_ffmpeg: "ffmpeg kurulu değil, yöneticiye haber ver.",
    err_timeout: "Çok uzun sürdü, bir daha dener misin?",
    err_local: "Bu bağlantıyı açamam (yerel ağ adresi).",
    err_unsupported: "Bu linki tanımıyorum.",
    err_other: "Olmadı, bunu açamadım 😕",
    play_what: "Ne çalayım? `{p}çal <şarkı adı ya da link>`",
    voice_first_call: "Önce bir ses kanalına gir, sonra çağır 🙂",
    voice_first: "Önce bir ses kanalına gir 🙂",
    skipped: "⏭ Geçtim.",
    nothing_playing: "Şu an bir şey çalmıyor.",
    stopped: "⏹ Durdurdum, sırayı temizledim. Görüşürüz 👋",
    stop_nothing: "Zaten bir şey çalmıyordu.",
    paused: "⏸ Duraklattım. `{p}devam` ile sürdürürüz.",
    pause_nothing: "Duraklatılacak bir şey yok.",
    resumed: "▶️ Devam ediyoruz.",
    resume_nothing: "Duraklatılmış bir şey yok.",
    volume_is: "🔊 Ses: {volume}",
    volume_set: "🔊 Ses {volume} oldu.",
    volume_range: "Ses 0 ile 200 arasında olmalı: `{p}ses 80`",
    shuffle_few: "Karıştıracak kadar şarkı yok 🙂",
    shuffled: "🔀 Sırayı karıştırdım ({songs}).",
    loop_usage: "Kullanım: `{p}tekrar [kapalı|şarkı|hepsi]`",
    coming: "🐼 Geliyorum!",
    already_here: "Zaten buradayım 🙂",
    left: "👋 Çıktım. `{p}gel` ile geri çağırabilirsin.",
    not_in_voice: "Zaten bir ses kanalında değilim.",
    remove_usage: "Kullanım: `{p}sil <sıra numarası>`",
    removed: "🗑 Sıradan çıkardım: **{title}**",
    no_such: "Sırada #{n} numaralı şarkı yok.",
    clear_empty: "Sıra zaten boş.",
    cleared: "🧹 Sırayı temizledim ({songs}).",
    unknown_button: "Bu düğmeyi tanımıyorum 🤔",
    join_forbidden: "O kanala girme iznim yok 🙁",
    join_plan_full: "Sunucunun ses kotası dolu, giremiyorum.",
    join_channel_full: "O kanal dolu ya da bir ses kanalı değil.",
    join_no_speak: "O kanalda konuşma iznim yok 🙁",
    join_failed: "Kanalına giremedim, biraz sonra tekrar dener misin?",
    queue_full: "Sıra dolu ({songs}), biraz bekle 🙂",
    track_failed: "**{title}** çalınamadı, geçiyorum. ({reason})",
    voice_lost: "Ses bağlantım koptu 😵 `{p}gel` yazarsan geri gelirim.",
    alone_leaving: "Kimse kalmadı, ben de çıkıyorum 👋",
    idle_leaving: "Bir süredir bir şey çalmıyorum, çıkıyorum 👋 `{p}çal` ile geri çağır.",
    panel_paused: "⏸ Duraklatıldı",
    panel_loop_one: "🔂 Şarkı tekrar",
    panel_loop_all: "🔁 Sıra tekrar",
    panel_queue_empty: "Sırada şarkı yok",
    panel_queued: "Sırada {songs}",
    button_pause: "Duraklat",
    button_resume: "Devam",
    button_skip: "Geç",
    button_stop: "Dur",
    button_shuffle: "Karıştır",
    button_loop_off: "Tekrar: kapalı",
    button_loop_one: "Tekrar: şarkı",
    button_loop_all: "Tekrar: hepsi",
    button_volume_down: "Ses −",
    button_volume_up: "Ses +",
    button_queue: "Sıra",
};

/// English.
pub static EN: Strings = Strings {
    untitled: "Untitled song",
    live: "live",
    now_label: "🐼 Now playing:",
    played_label: "🎵 Played:",
    requested_by: "requested by {name}",
    queued: "Added to the queue (#{n}): **{title}** ({length})",
    listening: "playing for {time}",
    now_details: "Requested by {name} · Volume: {volume}",
    now_paused: " · ⏸ paused",
    now_loop_one: " · 🔂 repeating this song",
    now_loop_all: " · 🔁 repeating the queue",
    queue_empty: "The queue is empty. Add something with `{p}play <song name or link>` 🎵",
    queue_no_more: "Nothing else in the queue.",
    queue_header: "{songs} in the queue ({time}):",
    queue_more: "…and {songs} more",
    queue_loop_one: "🔂 This song is on repeat.",
    queue_loop_all: "🔁 The queue is on repeat.",
    song_one: "{n} song",
    song_many: "{n} songs",
    percent: "{n}%",
    not_in_channel: "You need to be in my voice channel for that 🙂",
    loop_off: "Repeat is off.",
    loop_one: "🔂 This song will repeat.",
    loop_all: "🔁 The whole queue will repeat.",
    help: "🐼 **Panda commands**\n\
           `{p}play <song name or link>` (`{p}p`): plays or queues (YouTube, SoundCloud, file links)\n\
           `{p}skip` (`{p}next`): next song\n\
           `{p}stop`: stops, clears the queue and leaves\n\
           `{p}pause` · `{p}resume`\n\
           `{p}queue` (`{p}q`): shows the queue\n\
           `{p}np`: what is playing now\n\
           `{p}volume <0-200>` (`{p}vol`): volume\n\
           `{p}shuffle`: shuffles the queue\n\
           `{p}loop [off|one|all]` (`{p}repeat`)\n\
           `{p}remove <#>` · `{p}clear`\n\
           `{p}join`: comes to your channel · `{p}leave`: leaves the channel\n\
           `{p}help`: this list\n\
           The Turkish, French, German and Spanish names work too (`{p}çal`, `{p}jouer`, `{p}spielen`, `{p}reproducir`, …).",
    err_bot_check: "YouTube thinks I'm a bot 🙈 Try again in a bit (or ask the admin to add a cookies file).",
    err_age: "This video is age-restricted, I can't open it.",
    err_unavailable: "This video is unavailable (private or removed).",
    err_not_found: "I couldn't find anything 🤔",
    err_missing_ytdlp: "yt-dlp is not installed, please tell the admin.",
    err_missing_ffmpeg: "ffmpeg is not installed, please tell the admin.",
    err_timeout: "That took too long, could you try again?",
    err_local: "I can't open that link (local network address).",
    err_unsupported: "I don't know this kind of link.",
    err_other: "That didn't work, I couldn't open it 😕",
    play_what: "What should I play? `{p}play <song name or link>`",
    voice_first_call: "Join a voice channel first, then call me 🙂",
    voice_first: "Join a voice channel first 🙂",
    skipped: "⏭ Skipped.",
    nothing_playing: "Nothing is playing right now.",
    stopped: "⏹ Stopped and cleared the queue. See you 👋",
    stop_nothing: "Nothing was playing anyway.",
    paused: "⏸ Paused. `{p}resume` carries on.",
    pause_nothing: "There is nothing to pause.",
    resumed: "▶️ Back to it.",
    resume_nothing: "Nothing is paused.",
    volume_is: "🔊 Volume: {volume}",
    volume_set: "🔊 Volume set to {volume}.",
    volume_range: "The volume must be between 0 and 200: `{p}volume 80`",
    shuffle_few: "Not enough songs to shuffle 🙂",
    shuffled: "🔀 Shuffled the queue ({songs}).",
    loop_usage: "Usage: `{p}loop [off|one|all]`",
    coming: "🐼 On my way!",
    already_here: "I'm already here 🙂",
    left: "👋 I left. `{p}join` brings me back.",
    not_in_voice: "I'm not in a voice channel.",
    remove_usage: "Usage: `{p}remove <number in the queue>`",
    removed: "🗑 Removed from the queue: **{title}**",
    no_such: "There is no song #{n} in the queue.",
    clear_empty: "The queue is already empty.",
    cleared: "🧹 Cleared the queue ({songs}).",
    unknown_button: "I don't know this button 🤔",
    join_forbidden: "I'm not allowed to join that channel 🙁",
    join_plan_full: "The server's voice limit is reached, I can't join.",
    join_channel_full: "That channel is full or not a voice channel.",
    join_no_speak: "I'm not allowed to speak in that channel 🙁",
    join_failed: "I couldn't join your channel, could you try again in a moment?",
    queue_full: "The queue is full ({songs}), wait a bit 🙂",
    track_failed: "Couldn't play **{title}**, skipping it. ({reason})",
    voice_lost: "I lost my voice connection 😵 Type `{p}join` and I'll come back.",
    alone_leaving: "Nobody is left, so I'm leaving too 👋",
    idle_leaving: "Nothing has played for a while, so I'm leaving 👋 `{p}play` calls me back.",
    panel_paused: "⏸ Paused",
    panel_loop_one: "🔂 Song on repeat",
    panel_loop_all: "🔁 Queue on repeat",
    panel_queue_empty: "Queue empty",
    panel_queued: "{songs} queued",
    button_pause: "Pause",
    button_resume: "Resume",
    button_skip: "Skip",
    button_stop: "Stop",
    button_shuffle: "Shuffle",
    button_loop_off: "Repeat: off",
    button_loop_one: "Repeat: song",
    button_loop_all: "Repeat: all",
    button_volume_down: "Vol −",
    button_volume_up: "Vol +",
    button_queue: "Queue",
};

/// French (casual "tu", like the Turkish).
pub static FR: Strings = Strings {
    untitled: "Titre sans nom",
    live: "en direct",
    now_label: "🐼 En cours :",
    played_label: "🎵 Joué :",
    requested_by: "demandé par {name}",
    queued: "Ajouté à la file (#{n}) : **{title}** ({length})",
    listening: "en écoute depuis {time}",
    now_details: "Demandé par {name} · Volume : {volume}",
    now_paused: " · ⏸ en pause",
    now_loop_one: " · 🔂 ce titre en boucle",
    now_loop_all: " · 🔁 la file en boucle",
    queue_empty: "La file est vide. Ajoute quelque chose avec `{p}jouer <titre ou lien>` 🎵",
    queue_no_more: "Rien d'autre dans la file.",
    queue_header: "{songs} dans la file ({time}) :",
    queue_more: "…et {songs} de plus",
    queue_loop_one: "🔂 Ce titre tourne en boucle.",
    queue_loop_all: "🔁 La file tourne en boucle.",
    song_one: "{n} titre",
    song_many: "{n} titres",
    percent: "{n} %",
    not_in_channel: "Pour ça, tu dois être dans mon salon vocal 🙂",
    loop_off: "Répétition désactivée.",
    loop_one: "🔂 Ce titre va se répéter.",
    loop_all: "🔁 Toute la file va se répéter.",
    help: "🐼 **Commandes de Panda**\n\
           `{p}jouer <titre ou lien>` (`{p}play`, `{p}p`) : joue ou ajoute à la file (YouTube, SoundCloud, liens de fichiers)\n\
           `{p}passer` (`{p}skip`) : titre suivant\n\
           `{p}arrêter` (`{p}stop`) : arrête, vide la file et quitte le salon\n\
           `{p}pause` · `{p}reprendre` (`{p}resume`)\n\
           `{p}file` (`{p}queue`) : affiche la file\n\
           `{p}maintenant` (`{p}np`) : le titre en cours\n\
           `{p}volume <0-200>` : le volume\n\
           `{p}mélanger` (`{p}shuffle`) : mélange la file\n\
           `{p}boucle [non|titre|tout]` (`{p}loop [off|one|all]`)\n\
           `{p}retirer <n°>` (`{p}remove`) · `{p}vider` (`{p}clear`)\n\
           `{p}rejoindre` (`{p}join`) : vient dans ton salon · `{p}quitter` (`{p}leave`) : quitte le salon\n\
           `{p}aide` (`{p}help`) : cette liste",
    err_bot_check: "YouTube m'a pris pour un bot 🙈 Réessaie dans un moment (ou demande à l'admin d'ajouter un fichier de cookies).",
    err_age: "Cette vidéo est soumise à une limite d'âge, je ne peux pas l'ouvrir.",
    err_unavailable: "Cette vidéo n'est pas disponible (privée ou supprimée).",
    err_not_found: "Je n'ai rien trouvé 🤔",
    err_missing_ytdlp: "yt-dlp n'est pas installé, préviens l'admin.",
    err_missing_ffmpeg: "ffmpeg n'est pas installé, préviens l'admin.",
    err_timeout: "Ça a pris trop de temps, tu peux réessayer ?",
    err_local: "Je ne peux pas ouvrir ce lien (adresse du réseau local).",
    err_unsupported: "Je ne connais pas ce genre de lien.",
    err_other: "Ça n'a pas marché, je n'ai pas pu l'ouvrir 😕",
    play_what: "Je joue quoi ? `{p}jouer <titre ou lien>`",
    voice_first_call: "Rejoins d'abord un salon vocal, puis appelle-moi 🙂",
    voice_first: "Rejoins d'abord un salon vocal 🙂",
    skipped: "⏭ Passé.",
    nothing_playing: "Rien ne joue en ce moment.",
    stopped: "⏹ Arrêté, file vidée. À plus 👋",
    stop_nothing: "Rien ne jouait de toute façon.",
    paused: "⏸ En pause. `{p}reprendre` pour continuer.",
    pause_nothing: "Il n'y a rien à mettre en pause.",
    resumed: "▶️ C'est reparti.",
    resume_nothing: "Rien n'est en pause.",
    volume_is: "🔊 Volume : {volume}",
    volume_set: "🔊 Volume réglé à {volume}.",
    volume_range: "Le volume doit être entre 0 et 200 : `{p}volume 80`",
    shuffle_few: "Pas assez de titres à mélanger 🙂",
    shuffled: "🔀 File mélangée ({songs}).",
    loop_usage: "Utilisation : `{p}boucle [non|titre|tout]`",
    coming: "🐼 J'arrive !",
    already_here: "Je suis déjà là 🙂",
    left: "👋 Je suis parti. `{p}rejoindre` pour me rappeler.",
    not_in_voice: "Je ne suis dans aucun salon vocal.",
    remove_usage: "Utilisation : `{p}retirer <numéro dans la file>`",
    removed: "🗑 Retiré de la file : **{title}**",
    no_such: "Il n'y a pas de titre #{n} dans la file.",
    clear_empty: "La file est déjà vide.",
    cleared: "🧹 File vidée ({songs}).",
    unknown_button: "Je ne connais pas ce bouton 🤔",
    join_forbidden: "Je n'ai pas le droit de rejoindre ce salon 🙁",
    join_plan_full: "La limite vocale du serveur est atteinte, je ne peux pas entrer.",
    join_channel_full: "Ce salon est plein ou ce n'est pas un salon vocal.",
    join_no_speak: "Je n'ai pas le droit de parler dans ce salon 🙁",
    join_failed: "Je n'ai pas pu rejoindre ton salon, tu peux réessayer dans un instant ?",
    queue_full: "La file est pleine ({songs}), attends un peu 🙂",
    track_failed: "Impossible de jouer **{title}**, je passe. ({reason})",
    voice_lost: "J'ai perdu ma connexion vocale 😵 Tape `{p}rejoindre` et je reviens.",
    alone_leaving: "Il n'y a plus personne, je m'en vais aussi 👋",
    idle_leaving: "Rien ne joue depuis un moment, je m'en vais 👋 `{p}jouer` pour me rappeler.",
    panel_paused: "⏸ En pause",
    panel_loop_one: "🔂 Titre en boucle",
    panel_loop_all: "🔁 File en boucle",
    panel_queue_empty: "File vide",
    panel_queued: "{songs} en attente",
    button_pause: "Pause",
    button_resume: "Reprendre",
    button_skip: "Passer",
    button_stop: "Arrêter",
    button_shuffle: "Mélanger",
    button_loop_off: "Boucle : non",
    button_loop_one: "Boucle : titre",
    button_loop_all: "Boucle : tout",
    button_volume_down: "Vol −",
    button_volume_up: "Vol +",
    button_queue: "File",
};

/// German (casual "du").
pub static DE: Strings = Strings {
    untitled: "Unbenannter Titel",
    live: "live",
    now_label: "🐼 Läuft gerade:",
    played_label: "🎵 Lief:",
    requested_by: "gewünscht von {name}",
    queued: "In die Warteschlange gesetzt (#{n}): **{title}** ({length})",
    listening: "läuft seit {time}",
    now_details: "Gewünscht von {name} · Lautstärke: {volume}",
    now_paused: " · ⏸ pausiert",
    now_loop_one: " · 🔂 dieser Titel wird wiederholt",
    now_loop_all: " · 🔁 die Warteschlange wird wiederholt",
    queue_empty: "Die Warteschlange ist leer. Füg etwas hinzu mit `{p}spielen <Titel oder Link>` 🎵",
    queue_no_more: "Sonst ist nichts in der Warteschlange.",
    queue_header: "{songs} in der Warteschlange ({time}):",
    queue_more: "…und {songs} mehr",
    queue_loop_one: "🔂 Dieser Titel wird wiederholt.",
    queue_loop_all: "🔁 Die Warteschlange wird wiederholt.",
    song_one: "{n} Titel",
    song_many: "{n} Titel",
    percent: "{n} %",
    not_in_channel: "Dafür musst du in meinem Sprachkanal sein 🙂",
    loop_off: "Wiederholen ist aus.",
    loop_one: "🔂 Dieser Titel wird wiederholt.",
    loop_all: "🔁 Die ganze Warteschlange wird wiederholt.",
    help: "🐼 **Panda-Befehle**\n\
           `{p}spielen <Titel oder Link>` (`{p}play`, `{p}p`): spielt ab oder reiht ein (YouTube, SoundCloud, Datei-Links)\n\
           `{p}überspringen` (`{p}skip`): nächster Titel\n\
           `{p}stopp` (`{p}stop`): stoppt, leert die Warteschlange und geht\n\
           `{p}pause` · `{p}fortsetzen` (`{p}resume`)\n\
           `{p}warteschlange` (`{p}queue`, `{p}q`): zeigt die Warteschlange\n\
           `{p}jetzt` (`{p}np`): was gerade läuft\n\
           `{p}lautstärke <0-200>` (`{p}volume`): Lautstärke\n\
           `{p}mischen` (`{p}shuffle`): mischt die Warteschlange\n\
           `{p}wiederholen [aus|titel|alle]` (`{p}loop [off|one|all]`)\n\
           `{p}entfernen <#>` (`{p}remove`) · `{p}leeren` (`{p}clear`)\n\
           `{p}komm` (`{p}join`): kommt in deinen Kanal · `{p}verlassen` (`{p}leave`): verlässt den Kanal\n\
           `{p}hilfe` (`{p}help`): diese Liste",
    err_bot_check: "YouTube hält mich für einen Bot 🙈 Versuch es gleich noch mal (oder bitte den Admin, eine Cookie-Datei einzurichten).",
    err_age: "Dieses Video hat eine Altersbeschränkung, ich kann es nicht öffnen.",
    err_unavailable: "Dieses Video ist nicht verfügbar (privat oder entfernt).",
    err_not_found: "Ich habe nichts gefunden 🤔",
    err_missing_ytdlp: "yt-dlp ist nicht installiert, sag dem Admin Bescheid.",
    err_missing_ffmpeg: "ffmpeg ist nicht installiert, sag dem Admin Bescheid.",
    err_timeout: "Das hat zu lange gedauert, versuchst du es noch mal?",
    err_local: "Diesen Link kann ich nicht öffnen (lokale Netzwerkadresse).",
    err_unsupported: "Diese Art Link kenne ich nicht.",
    err_other: "Das hat nicht geklappt, ich konnte es nicht öffnen 😕",
    play_what: "Was soll ich spielen? `{p}spielen <Titel oder Link>`",
    voice_first_call: "Geh erst in einen Sprachkanal und ruf mich dann 🙂",
    voice_first: "Geh erst in einen Sprachkanal 🙂",
    skipped: "⏭ Übersprungen.",
    nothing_playing: "Gerade läuft nichts.",
    stopped: "⏹ Gestoppt, Warteschlange geleert. Bis dann 👋",
    stop_nothing: "Es lief sowieso nichts.",
    paused: "⏸ Pausiert. Mit `{p}fortsetzen` geht es weiter.",
    pause_nothing: "Es gibt nichts zu pausieren.",
    resumed: "▶️ Weiter geht's.",
    resume_nothing: "Nichts ist pausiert.",
    volume_is: "🔊 Lautstärke: {volume}",
    volume_set: "🔊 Lautstärke ist jetzt {volume}.",
    volume_range: "Die Lautstärke muss zwischen 0 und 200 liegen: `{p}lautstärke 80`",
    shuffle_few: "Zu wenige Titel zum Mischen 🙂",
    shuffled: "🔀 Warteschlange gemischt ({songs}).",
    loop_usage: "Verwendung: `{p}wiederholen [aus|titel|alle]`",
    coming: "🐼 Bin unterwegs!",
    already_here: "Ich bin schon da 🙂",
    left: "👋 Bin raus. Mit `{p}komm` holst du mich zurück.",
    not_in_voice: "Ich bin in keinem Sprachkanal.",
    remove_usage: "Verwendung: `{p}entfernen <Nummer in der Warteschlange>`",
    removed: "🗑 Aus der Warteschlange entfernt: **{title}**",
    no_such: "In der Warteschlange gibt es keinen Titel #{n}.",
    clear_empty: "Die Warteschlange ist schon leer.",
    cleared: "🧹 Warteschlange geleert ({songs}).",
    unknown_button: "Diesen Knopf kenne ich nicht 🤔",
    join_forbidden: "Ich darf diesen Kanal nicht betreten 🙁",
    join_plan_full: "Das Sprachlimit des Servers ist erreicht, ich komme nicht rein.",
    join_channel_full: "Der Kanal ist voll oder kein Sprachkanal.",
    join_no_speak: "Ich darf in diesem Kanal nicht sprechen 🙁",
    join_failed: "Ich konnte deinem Kanal nicht beitreten, versuchst du es gleich noch mal?",
    queue_full: "Die Warteschlange ist voll ({songs}), warte ein bisschen 🙂",
    track_failed: "**{title}** ließ sich nicht abspielen, ich überspringe ihn. ({reason})",
    voice_lost: "Meine Sprachverbindung ist weg 😵 Schreib `{p}komm`, dann komme ich zurück.",
    alone_leaving: "Keiner mehr da, ich gehe auch 👋",
    idle_leaving: "Seit einer Weile läuft nichts, ich gehe 👋 Mit `{p}spielen` holst du mich zurück.",
    panel_paused: "⏸ Pausiert",
    panel_loop_one: "🔂 Titel wiederholt",
    panel_loop_all: "🔁 Liste wiederholt",
    panel_queue_empty: "Warteschlange leer",
    panel_queued: "{songs} in der Warteschlange",
    button_pause: "Pause",
    button_resume: "Weiter",
    button_skip: "Überspringen",
    button_stop: "Stopp",
    button_shuffle: "Mischen",
    button_loop_off: "Wiederholen: aus",
    button_loop_one: "Wiederholen: Titel",
    button_loop_all: "Wiederholen: alle",
    button_volume_down: "Leiser",
    button_volume_up: "Lauter",
    button_queue: "Warteschlange",
};

/// Spanish (casual "tú").
pub static ES: Strings = Strings {
    untitled: "Canción sin título",
    live: "en directo",
    now_label: "🐼 Sonando ahora:",
    played_label: "🎵 Sonó:",
    requested_by: "pedida por {name}",
    queued: "Añadida a la cola (#{n}): **{title}** ({length})",
    listening: "sonando desde hace {time}",
    now_details: "Pedida por {name} · Volumen: {volume}",
    now_paused: " · ⏸ en pausa",
    now_loop_one: " · 🔂 repitiendo esta canción",
    now_loop_all: " · 🔁 repitiendo la cola",
    queue_empty: "La cola está vacía. Añade algo con `{p}reproducir <canción o enlace>` 🎵",
    queue_no_more: "No hay nada más en la cola.",
    queue_header: "{songs} en la cola ({time}):",
    queue_more: "…y {songs} más",
    queue_loop_one: "🔂 Esta canción se repite.",
    queue_loop_all: "🔁 La cola se repite.",
    song_one: "{n} canción",
    song_many: "{n} canciones",
    percent: "{n} %",
    not_in_channel: "Para eso tienes que estar en mi canal de voz 🙂",
    loop_off: "Repetición desactivada.",
    loop_one: "🔂 Esta canción se repetirá.",
    loop_all: "🔁 Toda la cola se repetirá.",
    help: "🐼 **Comandos de Panda**\n\
           `{p}reproducir <canción o enlace>` (`{p}play`, `{p}p`): reproduce o añade a la cola (YouTube, SoundCloud, enlaces a archivos)\n\
           `{p}saltar` (`{p}skip`): siguiente canción\n\
           `{p}parar` (`{p}stop`): para, vacía la cola y se va\n\
           `{p}pausa` (`{p}pause`) · `{p}reanudar` (`{p}resume`)\n\
           `{p}cola` (`{p}queue`, `{p}q`): muestra la cola\n\
           `{p}ahora` (`{p}np`): lo que suena ahora\n\
           `{p}volumen <0-200>` (`{p}volume`): volumen\n\
           `{p}mezclar` (`{p}shuffle`): mezcla la cola\n\
           `{p}repetir [no|una|todo]` (`{p}loop [off|one|all]`)\n\
           `{p}quitar <#>` (`{p}remove`) · `{p}vaciar` (`{p}clear`)\n\
           `{p}ven` (`{p}join`): viene a tu canal · `{p}salir` (`{p}leave`): sale del canal\n\
           `{p}ayuda` (`{p}help`): esta lista",
    err_bot_check: "YouTube cree que soy un bot 🙈 Vuelve a intentarlo en un rato (o pide al admin que añada un archivo de cookies).",
    err_age: "Este vídeo tiene restricción de edad, no puedo abrirlo.",
    err_unavailable: "Este vídeo no está disponible (privado o eliminado).",
    err_not_found: "No he encontrado nada 🤔",
    err_missing_ytdlp: "yt-dlp no está instalado, avisa al admin.",
    err_missing_ffmpeg: "ffmpeg no está instalado, avisa al admin.",
    err_timeout: "Ha tardado demasiado, ¿lo intentas otra vez?",
    err_local: "No puedo abrir ese enlace (dirección de red local).",
    err_unsupported: "No conozco este tipo de enlace.",
    err_other: "No ha funcionado, no he podido abrirlo 😕",
    play_what: "¿Qué pongo? `{p}reproducir <canción o enlace>`",
    voice_first_call: "Entra primero en un canal de voz y luego llámame 🙂",
    voice_first: "Entra primero en un canal de voz 🙂",
    skipped: "⏭ Saltada.",
    nothing_playing: "Ahora mismo no suena nada.",
    stopped: "⏹ He parado y vaciado la cola. ¡Hasta luego! 👋",
    stop_nothing: "No sonaba nada de todos modos.",
    paused: "⏸ En pausa. `{p}reanudar` para seguir.",
    pause_nothing: "No hay nada que pausar.",
    resumed: "▶️ Seguimos.",
    resume_nothing: "No hay nada en pausa.",
    volume_is: "🔊 Volumen: {volume}",
    volume_set: "🔊 Volumen al {volume}.",
    volume_range: "El volumen debe estar entre 0 y 200: `{p}volumen 80`",
    shuffle_few: "No hay suficientes canciones para mezclar 🙂",
    shuffled: "🔀 He mezclado la cola ({songs}).",
    loop_usage: "Uso: `{p}repetir [no|una|todo]`",
    coming: "🐼 ¡Voy!",
    already_here: "Ya estoy aquí 🙂",
    left: "👋 Me he ido. `{p}ven` para volver a llamarme.",
    not_in_voice: "No estoy en ningún canal de voz.",
    remove_usage: "Uso: `{p}quitar <número en la cola>`",
    removed: "🗑 Quitada de la cola: **{title}**",
    no_such: "No hay ninguna canción #{n} en la cola.",
    clear_empty: "La cola ya está vacía.",
    cleared: "🧹 He vaciado la cola ({songs}).",
    unknown_button: "No conozco este botón 🤔",
    join_forbidden: "No tengo permiso para entrar en ese canal 🙁",
    join_plan_full: "Se ha alcanzado el límite de voz del servidor, no puedo entrar.",
    join_channel_full: "Ese canal está lleno o no es un canal de voz.",
    join_no_speak: "No tengo permiso para hablar en ese canal 🙁",
    join_failed: "No he podido entrar en tu canal, ¿lo intentas en un momento?",
    queue_full: "La cola está llena ({songs}), espera un poco 🙂",
    track_failed: "No he podido reproducir **{title}**, la salto. ({reason})",
    voice_lost: "Se me cortó la conexión de voz 😵 Escribe `{p}ven` y vuelvo.",
    alone_leaving: "No queda nadie, yo también me voy 👋",
    idle_leaving: "Hace rato que no suena nada, me voy 👋 `{p}reproducir` para volver a llamarme.",
    panel_paused: "⏸ En pausa",
    panel_loop_one: "🔂 Canción en bucle",
    panel_loop_all: "🔁 Cola en bucle",
    panel_queue_empty: "Cola vacía",
    panel_queued: "{songs} en cola",
    button_pause: "Pausa",
    button_resume: "Seguir",
    button_skip: "Saltar",
    button_stop: "Parar",
    button_shuffle: "Mezclar",
    button_loop_off: "Repetir: no",
    button_loop_one: "Repetir: una",
    button_loop_all: "Repetir: todo",
    button_volume_down: "Vol −",
    button_volume_up: "Vol +",
    button_queue: "Cola",
};

/// The texts of `lang`.
#[must_use]
pub const fn strings(lang: Locale) -> &'static Strings {
    match lang {
        Locale::Tr => &TR,
        Locale::En => &EN,
        Locale::De => &DE,
        Locale::Fr => &FR,
        Locale::Es => &ES,
    }
}

/// `PANDA_LANG`: `tr`, `en`, `fr`, `de` or `es` (a region such as `fr-CA`
/// is fine).
pub fn parse(value: &str) -> Result<Locale, String> {
    Locale::supported(value).ok_or_else(|| {
        format!(
            "not a language Panda speaks: {value} (use {})",
            Locale::ALL.map(Locale::code).join(", ")
        )
    })
}

/// Fills `{name}` placeholders in one pass (so a value that contains
/// braces, such as a song title, is never filled again). Unknown or
/// unclosed placeholders stay as they are.
#[must_use]
pub fn fill(template: &str, args: &[(&str, &dyn std::fmt::Display)]) -> String {
    let mut out = String::with_capacity(template.len() + 32);
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let value = after.find('}').and_then(|close| {
            let name = &after[..close];
            args.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (close, value))
        });
        match value {
            Some((close, value)) => {
                let _ = write!(out, "{value}");
                rest = &after[close + 1..];
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// `3 songs`, `1 titre`, `5 şarkı`.
#[must_use]
pub fn songs(lang: Locale, count: usize) -> String {
    let s = strings(lang);
    let template = if count == 1 { s.song_one } else { s.song_many };
    fill(template, &[("n", &count)])
}

/// `%80`, `80%`, `80 %`.
#[must_use]
pub fn percent(lang: Locale, value: u32) -> String {
    fill(strings(lang).percent, &[("n", &value)])
}

/// The `{name}` placeholders of a template, in order.
#[must_use]
pub fn placeholders(template: &str) -> Vec<&str> {
    let mut names = Vec::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close)
                if !after[..close].is_empty()
                    && after[..close]
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_') =>
            {
                names.push(&after[..close]);
                rest = &after[close + 1..];
            }
            _ => rest = after,
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn languages_are_parsed() {
        assert_eq!(parse("fr"), Ok(Locale::Fr));
        assert_eq!(parse("EN"), Ok(Locale::En));
        assert_eq!(parse("de-AT"), Ok(Locale::De));
        assert_eq!(parse("es_MX"), Ok(Locale::Es));
        assert_eq!(parse("tr"), Ok(Locale::Tr));
        let error = parse("klingon").unwrap_err();
        assert!(error.contains("tr, en, de, fr, es"), "{error}");
    }

    #[test]
    fn templates_are_filled_once() {
        assert_eq!(fill("{a} and {b}", &[("a", &"x"), ("b", &2)]), "x and 2");
        // A value with braces is not filled again; unknown names stay.
        assert_eq!(fill("**{title}**", &[("title", &"{p}")]), "**{p}**");
        assert_eq!(fill("{nope} {", &[]), "{nope} {");
        assert_eq!(placeholders("`{p}play` {songs} {"), ["p", "songs"]);
    }

    #[test]
    fn counts_and_percentages() {
        assert_eq!(songs(Locale::Tr, 1), "1 şarkı");
        assert_eq!(songs(Locale::En, 1), "1 song");
        assert_eq!(songs(Locale::En, 3), "3 songs");
        assert_eq!(songs(Locale::Fr, 2), "2 titres");
        assert_eq!(songs(Locale::Es, 2), "2 canciones");
        assert_eq!(songs(Locale::De, 2), "2 Titel");
        assert_eq!(percent(Locale::Tr, 80), "%80");
        assert_eq!(percent(Locale::En, 80), "80%");
        assert_eq!(percent(Locale::Fr, 80), "80 %");
    }

    /// Every language says everything, with the Turkish text's
    /// placeholders (in any order, any number of times).
    #[test]
    fn every_language_has_every_text() {
        let base = TR.fields();
        for lang in Locale::ALL {
            let fields = strings(lang).fields();
            assert_eq!(fields.len(), base.len());
            for ((name, text), (_, tr)) in fields.iter().zip(&base) {
                assert!(!text.trim().is_empty(), "{lang:?}.{name} is empty");
                let have: std::collections::BTreeSet<_> = placeholders(text).into_iter().collect();
                let want: std::collections::BTreeSet<_> = placeholders(tr).into_iter().collect();
                assert_eq!(
                    have, want,
                    "{lang:?}.{name}: placeholders differ from Turkish"
                );
            }
        }
    }

    /// Button labels stay short (the server allows 80 characters; the panel
    /// should fit a phone).
    #[test]
    fn button_labels_are_short() {
        for lang in Locale::ALL {
            for (name, text) in strings(lang).fields() {
                if name.starts_with("button_") {
                    assert!(text.chars().count() <= 20, "{lang:?}.{name}: {text}");
                }
            }
        }
    }
}
