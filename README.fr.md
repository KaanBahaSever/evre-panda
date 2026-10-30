# Panda : le bot musical pour Evre

Panda joue de la musique dans les salons vocaux de ton serveur
[Evre](https://evre.chat) : YouTube, SoundCloud et tout site que
[yt-dlp](https://github.com/yt-dlp/yt-dlp) connaît, ou des liens directs
vers des fichiers audio. File d'attente, panneau « En cours » avec boutons,
boucle, mélange et volume normalisé.

**Tu fais tourner Panda toi-même** (ton PC, un serveur à la maison, un
VPS) : Evre n'héberge pas les bots. Panda se connecte seulement à Evre (il
n'ouvre aucun port). La version complète de ce guide, en anglais :
[README.md](README.md). Guide sur le site : <https://evre.chat/fr/panda/>

## Il te faut

- Un serveur Evre **avec une offre payante** : l'offre Gratuit n'autorise
  aucun bot (Standard : 3 bots, Pro : 10, Communauté : 25).
- Le droit de gérer le serveur (pour créer le bot).
- Windows 10/11 x64 ou Linux x86_64 (Ubuntu 20.04+, Debian 11+). Sur macOS,
  il faut compiler Panda soi-même.
- [yt-dlp](https://github.com/yt-dlp/yt-dlp) et [FFmpeg](https://ffmpeg.org)
  (avec `ffprobe`).

## Installation

1. **Crée le bot** : dans Evre, ouvre ton serveur, puis **Paramètres du
   serveur → Bots → Créer un bot**, nomme-le `Panda`. Evre affiche le
   **jeton** (`erabot_…`) **une seule fois** : copie-le, puis clique sur
   **Je l’ai noté**. Le jeton est le mot de passe du bot : ne le partage
   jamais. Perdu ou divulgué ? **Régénérer le jeton**.
2. **Télécharge Panda** et décompresse-le dans un dossier à part :
   [Windows](https://evre.chat/downloads/panda/panda-windows-x86_64.zip) ·
   [Linux](https://evre.chat/downloads/panda/panda-linux-x86_64.tar.gz).
   Si Windows SmartScreen avertit : **Informations complémentaires →
   Exécuter quand même**.
3. **Installe yt-dlp et FFmpeg** :
   - Windows : `winget install yt-dlp.yt-dlp` puis `winget install Gyan.FFmpeg`
     (ouvre ensuite un nouveau terminal) ;
   - Debian/Ubuntu : `sudo apt install ffmpeg`, et yt-dlp depuis ses
     propres versions :
     `curl -L https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp -o ~/.local/bin/yt-dlp && chmod +x ~/.local/bin/yt-dlp`
     (avec ce fichier, `PANDA_YTDLP_AUTO_UPDATE=true` le tient à jour) ;
   - macOS : `brew install yt-dlp ffmpeg`.
4. **Configure** : copie `panda.env.example` en `panda.env` et remplis-le :

   ```ini
   ERA_URL=https://evre.chat
   ERA_BOT_TOKEN=erabot_...ton jeton...
   PANDA_LANG=fr
   ```

5. **Lance-le** : `panda --check` (vérifie FFmpeg, yt-dlp et la voix), puis
   `panda --config panda.env` (sous Windows `.\panda.exe …`).
   `Panda is online as …` dans le journal : c'est connecté.
6. **Essaie** : rejoins un salon vocal, puis écris dans un salon textuel
   `!jouer lofi`.

## Commandes

Toutes les langues marchent en même temps ; les accents sont facultatifs.

| Commande | Aussi | Effet |
|---|---|---|
| `!jouer <titre ou lien>` | `!play`, `!p` | Joue ou ajoute à la file |
| `!passer` | `!suivant`, `!skip` | Titre suivant |
| `!arrêter` | `!stop` | Arrête, vide la file et quitte le salon |
| `!pause` · `!reprendre` | `!resume` | Pause, reprise |
| `!file` | `!liste`, `!queue` | Affiche la file |
| `!maintenant` | `!np` | Le titre en cours |
| `!volume [0-200]` | `!vol` | Le volume (100 = normal) |
| `!mélanger` | `!shuffle` | Mélange la file |
| `!boucle [non\|titre\|tout]` | `!loop` | Répétition |
| `!rejoindre` · `!quitter` | `!join` · `!leave` | Vient dans ton salon, le quitte |
| `!retirer <n°>` · `!vider` | `!remove` · `!clear` | Retire un titre, vide la file |
| `!aide` | `!help` | La liste des commandes |

Les commandes qui changent la lecture (et les boutons du panneau)
demandent d'être dans le salon vocal de Panda.

## En cas de problème

- **« YouTube m'a pris pour un bot »** : mets yt-dlp à jour (`yt-dlp -U`).
  Si ça continue, exporte les cookies YouTube d'un navigateur connecté
  (format Netscape) dans `cookies.txt` et ajoute
  `PANDA_YTDLP_COOKIES=cookies.txt`. Les VPS sont bloqués plus souvent
  qu'une connexion à la maison.
- **« ffmpeg n'est pas installé »** : `panda --check` dit ce qui manque.
  `ffmpeg -version` et `ffprobe -version` doivent marcher dans un nouveau
  terminal, sinon indique le chemin complet dans `PANDA_FFMPEG`.
- **Jeton refusé** : régénère-le dans Paramètres du serveur → Bots.

## Licence

Au choix : Apache 2.0 ou MIT (`LICENSE-APACHE`, `LICENSE-MIT`).
