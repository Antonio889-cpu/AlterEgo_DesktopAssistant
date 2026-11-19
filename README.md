# Alter Ego – Desktop Assistant

🇬🇧 **English** · 🇮🇹 [Italiano](README.it.md)

**🌐 Website: [alterego.attentoweb.it](https://alterego.attentoweb.it)** · **🎬 [Trailer](#trailer)**

> [!IMPORTANT]
> **Fan project · disclaimer**
>
> Alter Ego and Alter Net are unofficial fan projects. They are not affiliated with, endorsed or sponsored by Spike Chunsoft.
> Danganronpa, its characters and its original artwork are © Spike Chunsoft and belong to their owners.
>
> Both are free: nothing is sold, and the series' characters and art are used only as a tribute, never to make money.
> Ko-fi donations support me as a developer, not the project.
>
> If you hold the rights and want something changed or removed, [contact me on GitHub](https://github.com/Antonio889-cpu/AlterEgo_DesktopAssistant/issues) and I'll take it down right away.
>
> I'm a huge fan of the series and I bought every game. Thank you to Spike Chunsoft, Kazutaka Kodaka, Rui Komatsuzaki and everyone
> who cooked up this beautiful franchise: this project exists because of you. ♥

A desktop assistant for **Windows 10/11** and **Linux** inspired by *Alter Ego* from Danganronpa: a small CRT-laptop-style window,
always on top, that you talk to by voice or text. He/She answers with his/her own personality, remembers who you are,
saves game clips with a phrase, plays music, searches the web and, if you allow it, performs actions on your PC.

Interface in English and Italian.

## Trailer

<a href="https://alterego.attentoweb.it/trailer.html"><img src="docs/video/preview_en.gif" alt="Alter Ego trailer — click to watch it with sound" width="100%"></a>

**▶ [Watch the trailer with sound](https://alterego.attentoweb.it/trailer.html)** · [Italian version](https://alterego.attentoweb.it/trailer.html?lang=it) · [Full screen or in the site's frame](https://alterego.attentoweb.it/#trailer)

![Alter Ego on the desktop](docs/screenshots/en/main.png)

![Alter Ego's 8 expressions](docs/screenshots/expressions.png)

## Contents

- [Trailer](#trailer)
- [Features](#features)
- [Screenshots](#screenshots)
- [Installation](#installation)
- [First run](#first-run)
- [Usage](#usage)
- [AI services and API keys](#ai-services-and-api-keys)
- [Voice](#voice)
- [Mic-Link](#mic-link)
- [Music](#music)
- [Web search](#web-search)
- [Token saving](#token-saving)
- [Local models](#local-models)
- [Alter Net](#alter-net)
- [Costs](#costs)
- [Voice clips](#voice-clips)
- [PC actions and permissions](#pc-actions-and-permissions)
- [Personality and memory](#personality-and-memory)
- [Sprites](#sprites)
- [Minecraft: play together](#minecraft-play-together)
- [Pet](#pet)
- [Privacy and security](#privacy-and-security)
- [Where the data lives](#where-the-data-lives)
- [Troubleshooting](#troubleshooting)
- [Development](#development)
- [Known limitations](#known-limitations)
- [Credits and licenses](#credits-and-licenses)

## Features

- **Alter Ego-style window**: green screen with scanlines, glowing frame, logo, pixel dialogue box with typewriter effect and ▼.
  Borderless, transparent, always on top, hidden from the taskbar; it can be moved, resized (16:9) and remembers its position on every monitor.
- Optional **click-through**: the mouse goes through the window, which never steals focus while you play.
- **Old-TV animation** when the window opens (a bright line that expands) and closes (the screen collapses into a line, then a dot).
- **Automatic dialogue**: if you don't click, the text moves on by itself after a few seconds (and waits for the voice to finish).
- **English and Italian interface** (automatic from the Windows language, or your choice); Alter Ego's answers can follow the interface language.
- **5 AI services** (Gemini, OpenRouter, Groq, DeepSeek, local Ollama) with **automatic fallback** and **several API keys per service** in rotation.
  Prompt and history are built to make the most of the **DeepSeek cache** (cached input is much cheaper).
- **Push-to-talk** with Whisper transcription (Groq or a local server), on the keyboard or the **side mouse buttons**.
- Alter Ego's **voice**: **local Kokoro-82M** (offline, one-click install), Windows voices or any OpenAI-compatible service; dialogue **blips**.
- **Choice of audio devices**: speakers/headphones for the voice, microphone for push-to-talk.
- **Mic-Link**: a virtual microphone, **AlterEgo Link Audio**, with your voice + Alter Ego's, for Discord, OBS and games.
- **Music**: "play *title*" → Alter Ego searches YouTube and plays it in a CRT-style player with a spectrum, synced lyrics
  and his/her face changing to the beat. While music plays, Alter Ego's window shrinks and he/she puts on his/her headphones.
- **Music Venue**: a small library with recently played tracks and favorites, search and replay.
- **Talk to the app**: "open music venue", "close the audio player", "open the website", "check for updates"… everything about the app can be done by asking him/her.
- **Mouse and keyboard**: if you allow it, Alter Ego moves the cursor, clicks, types and presses keys (with confirmation and an emergency stop).
  Precise clicks: screenshots with numbered rulers and the cursor marked, zoom on small targets, a check screenshot after each click and, on Windows, the exact position of buttons and fields (UI Automation).
- **SQL Server** on your PC: when needed he/she asks for the connection details in a small form (the password never goes to the AI model), reads data, and with your permission changes it; results also show up in a SQL window where you can write your own queries. **LocalDB** works too: server `(localdb)\MSSQLLocalDB`.
- **Complex requests**: he/she chains tools by himself/herself (terminal, files, web, screen, database) for up to 12 steps (adjustable), checks each result and changes approach when something fails.
- **Noise suppression** for the Mic-Link microphone (RNNoise, adjustable).
- **Updates**: website and GitHub links in the app and a button to update to the latest release.
- **Web search** with your choice of engine: DeepSeek, Google (via Gemini), Tavily, Brave or DuckDuckGo (free), with fallback.
- **Spotify** (Premium): "play *title*" can play on your Spotify instead of YouTube.
- **Token saving**: thinking off, tools only when needed, local router (keywords or Rizzo Flow), cheap model for simple questions.
- **Costs panel**: every request with tokens, cache, duration and cost; daily/monthly totals and per-model summary, editable prices, CSV export.
- **Local models with Ollama**: **LFM3** by Liquid AI in one click, model browser with filters (abliterated, uncensored, tools, vision…),
  download from Hugging Face or any `.gguf` address, Ollama installed for you, **AMD Radeon** support (RX 6600 and newer, Vulkan) also on PCs with an Intel iGPU.
- **Fewer tokens in complex reasoning**: tools in groups loaded on demand, old tool results compacted, automatic thinking, project maps with Graphify.
- **Knows which apps are open** (names only), without breaking the provider cache.
- **Terminal**: Alter Ego can run PowerShell or cmd commands (bash on Linux), with confirmation or autonomously; he/she keeps the working folder between commands.
- **Full log** in Settings → Log: model requests, fallbacks, tools with result and duration, errors. **Silent start**: at login he/she starts hidden in the tray.
- **Voice clips**: "Alter Ego clip that" → saves the clip with AMD Adrenalin, NVIDIA App or Medal, **without going through the AI**.
- **PC actions** through tool calling, all off by default, with confirmation for sensitive ones (can be turned off) and a browsable log.
- **Local memory** (SQLite): remembers conversations, writes summaries, learns facts about you.
- **Adjustable personality** (shy ↔ confident, formal ↔ affectionate), **pronouns** (she, he or they), imitation mode, answers in Italian, English or both.
- Facial expressions driven by the answers (happy, surprised, embarrassed, sad, worried, thinking, error).
- **Pet mode**: in quiet moments Alter Ego comes out of the TV as a little pixel sprite and lives on your desktop: he/she walks, jumps, stands on open windows and on the taskbar, and you can grab and throw him/her. He/She goes back in when you talk to him/her.
- Lightweight: Tauri 2 (Rust) + vanilla HTML/CSS/JS, installer of about 6 MB.

## Screenshots

| | |
|---|---|
| ![Surprised](docs/screenshots/en/surprised.png) | ![Thinking](docs/screenshots/thinking.png) |
| Expression from the AI answer (`[surprised]`) | Thinking, scanlines only |
| ![Dark dialogue box and circuit background](docs/screenshots/en/happy-dark.png) | ![Push-to-talk: listening](docs/screenshots/en/listening.png) |
| Dark box and circuit background | Push-to-talk: listening |
| ![Permission request](docs/screenshots/en/confirm.png) | ![Text input](docs/screenshots/en/input.png) |
| Confirmation before a PC action | Writing to Alter Ego (`Ctrl+Alt+T`) |

**Music player and Music Venue**

| | |
|---|---|
| ![Alter Ego with headphones while music plays](docs/screenshots/en/main-music.png) | ![Music player: spectrum, title, controls, favorite, Venue button, synced lyrics](docs/screenshots/en/player.png) |
| While music plays he/she wears headphones (sprite by [Ayari](https://www.instagram.com/xxayariii/)) | Player: favorite ♥, Music Venue button, resizable corner |
| ![Music Venue: recent tracks, favorites and now playing](docs/screenshots/en/venue.png) | ![Music Venue: search in the library or on YouTube](docs/screenshots/en/venue-search.png) |
| Music Venue: recent tracks, favorites, now playing | Search in the library or on YouTube |

![The player expands into the Music Venue in the same window, with a glitch transition](docs/screenshots/en/venue-morph.png)

*The player expands into the Music Venue (and back) in the same window.*

![Alter Ego, the player and the detached Music Venue side by side, without overlapping](docs/screenshots/en/desktop-layout.png)

*Detached (⧉): the windows open side by side and never cover Alter Ego or each other.*

**"Old TV" power off and on**

![Animation: the screen collapses into a line and a dot, then turns back on from a bright line](docs/screenshots/tv-animation.png)

**Settings**

![Settings in a large window: the dashboard with Home, Integrations and LLM Library](docs/screenshots/en/settings-dashboard.png)

| | |
|---|---|
| ![AI & keys](docs/screenshots/en/settings-ai.png) | ![Hotkeys](docs/screenshots/en/settings-hotkeys.png) |
| AI services, several keys, fallback order | Hotkeys (mouse too) |
| ![Permissions](docs/screenshots/en/settings-perms.png) | ![Personality](docs/screenshots/en/settings-persona.png) |
| Action permissions | Personality and language |
| ![Voice & audio](docs/screenshots/en/settings-voice.png) | ![Costs](docs/screenshots/en/settings-costs.png) |
| Devices, Kokoro, music, Mic-Link | Cost of every request and prices |
| ![General](docs/screenshots/en/settings-general.png) | ![Voice clips](docs/screenshots/en/settings-clip.png) |
| Look, interface language, TV animation, automatic dialogue | Voice clips |
| ![Memory](docs/screenshots/en/settings-memory.png) | ![Log](docs/screenshots/en/settings-log.png) |
| Memory | Action log |

## Installation

**Windows**

1. From the [Releases](../../releases) page download the latest release: the **installer** `AlterEgo_<version>_setup.exe`
   or the **portable zip** (extract it and run `alter-ego.exe`). When updating from an older version, settings, keys and memory are kept.
2. Windows SmartScreen may warn that the app is not signed: *More info → Run anyway*.
3. **WebView2** is required; it is already included in up-to-date Windows 10/11.

**Linux** (x86-64; first version, less tested than Windows)

- **AppImage**: `chmod +x AlterEgo_<version>_amd64.AppImage` and run it. It updates itself from the settings.
- **Debian/Ubuntu**: `sudo apt install ./AlterEgo_<version>_amd64.deb`.
- Keys need a keyring (GNOME Keyring or KWallet), usually already there.
- Optional: `espeak-ng` (system voice), `xdotool` (mouse and keyboard, X11 only), `grim` or `gnome-screenshot` (screenshots), `yt-dlp`
  (otherwise Alter Ego downloads it), `unzip`.
- Windows only: Mic-Link (virtual audio cable) and Windows voices.

The app shows up in the **system tray** (next to the clock), not in the taskbar.

## First run

1. Right-click the tray icon → **Settings…**
2. **General**: enter your name and, if you like, pick the interface language (automatic = Windows language).
3. **AI & keys**: enable at least one service and paste an API key (for example [Groq](https://console.groq.com/keys)
   or [Google AI Studio](https://aistudio.google.com/apikey)), then *Test connection*. With Groq, voice input works too.
   Or use [Ollama](https://ollama.com) locally, with no keys.
4. Press `Ctrl+Alt+T` and type, or hold `Ctrl+Alt+Space` and talk.

## Usage

| What | How |
|---|---|
| Write | `Ctrl+Alt+T` (or tray → *Write to Alter Ego…*), Enter sends, Esc closes |
| Talk | hold push-to-talk (default `Ctrl+Alt+Space`), talk, release |
| Advance the dialogue | click the box, Enter or Space (or wait: it advances by itself) |
| Move / resize | drag / ↘ corner or `Ctrl` + wheel |
| Show / hide | `Ctrl+Alt+A` or left-click the tray icon |
| Click-through | `Ctrl+Alt+C` or tray; to leave it click the lock in the top-right corner |
| Quit | tray → *Quit* (Alt+F4 only hides) |

Automatic dialogue and the TV animation are turned on/off in **Settings → General**: the page's reading time is added to the chosen wait.

All hotkeys can be changed in **Settings → Hotkeys**: click the field and press the combination (a single key like `F8` works too).
For push-to-talk the **middle button** and the **side mouse buttons** (`Mouse4`, `Mouse5`) work as well.

The **interface language** (window, settings, player, tray menu and messages) is chosen in *Settings → General*:
automatic (Windows language), Italian or English. In *Personality*, "Answer language → Automatic" makes Alter Ego answer
in the same language.

From the tray you can also change opacity, background (scanlines, circuits, both), intensity, box style (light or dark), animated effects, sounds and voice.

## AI services and API keys

| Service | Notes |
|---|---|
| Google Gemini | OpenAI-compatible endpoint |
| OpenRouter | any model in the catalog |
| Groq | very fast; its keys are also used for Whisper (voice input) |
| DeepSeek | default model `deepseek-flash`; optimized for its cache (see below); its keys are also used for web search |
| Ollama | local, no key, no data leaves the PC |

- **Several keys per service**: if a key answers 401, 402, 403, 429 or "quota", the next one is used right away.
  Next time it starts from the last one that worked.
- **Fallback**: change the order of the services with ▲▼. If one doesn't answer (network, server error, no valid key), the next one is tried.
- Timeouts and 2 retries with increasing waits on temporary errors. If nobody answers, Alter Ego says so with his/her error expression.
  **Esc** stops an answer that is taking too long; a tool that hangs is stopped after a while, and the same call repeated over and over is blocked.
- **Advanced settings** (Settings → AI & keys): max tokens per answer, top P, max wait for the answer, max tool steps.
  For **Ollama**: context window (`num_ctx`), how long the model stays loaded and GPU layers (these use Ollama's native API).
- *Load models* downloads the list of models from the service: the default names may no longer exist.
- Keys are stored in the **Windows Credential Manager** (entry `AlterEgo`; on Linux in the system keyring), never in files or logs, and the frontend never sees them.

**DeepSeek cache** (and other services with automatic prefix caching). DeepSeek charges much less for input tokens
that start exactly like a previous request. That's why:

- the system prompt puts everything that never changes first (character, format, rules) and the parts that change last
  (name, facts about you, summary), so the opening block stays identical even when the memory updates;
- the history doesn't "slide" by one message every question: the window moves **in steps** of half its size,
  so for several turns in a row the whole previous conversation is already cached (between 1× and 1.5× the *Context messages* are sent);
- *Settings → AI & keys* shows how many input tokens came from the cache since the app was opened.

## Voice

- **Speech recognition (push-to-talk)**: Groq Whisper (`whisper-large-v3-turbo`), **Google Gemini** (Gemini key) or a local OpenAI-compatible
  Whisper server (e.g. faster-whisper-server/speaches, whisper.cpp server), with a **fallback** service (e.g. Groq ⇄ Gemini) if the first one fails. *Test microphone* records 4 seconds and shows what it understood.
- **Alter Ego's voice (TTS)**, in *Settings → Voice & audio*:
  - **Kokoro-82M (local)**: an 82M-parameter model ([Hugging Face](https://huggingface.co/hexgrad/Kokoro-82M),
    [library](https://github.com/hexgrad/kokoro), Apache-2.0) with English voices (*Heart*, *Bella*, *Emma*…) and the Italian
    *Sara* and *Nicola*. **Install Kokoro** opens a PowerShell window that creates a dedicated Python environment in
    `%APPDATA%\AlterEgo\kokoro` (installs Python 3.12 with winget if missing, PyTorch CPU or CUDA, the `kokoro` library)
    and downloads the model. Then Alter Ego starts the server by itself (only on `127.0.0.1`) and closes it when you quit.
    Already have [Kokoro-FastAPI](https://github.com/remsky/Kokoro-FastAPI)? Enter its URL and turn off autostart.
  - **Windows voices**, offline (more voices: Windows Settings → Time & language → Speech → Add voices);
  - any **OpenAI-compatible** `/audio/speech` service (e.g. local Piper or a cloud service);
  - **[VoiceStudio](https://github.com/debpalash/VoiceStudio)** (AGPL-3.0, local voices and voice cloning): *Use VoiceStudio* installs it
    with the official script if missing (with confirmation), starts it, points the voice at `http://127.0.0.1:3900/v1` and can start it together with Alter Ego.

  Sentences are synthesized one at a time: the first one plays right away, the others are prepared while he/she talks. His/her face moves while the voice plays.
- **Devices**: choose the output for voice and sounds and the push-to-talk microphone (or keep the Windows defaults).
- **Dialogue blips**: Danganronpa style (recreated with audio synthesis, no original files), soft blips, or muted.

## Mic-Link

Mic-Link creates a virtual microphone, **AlterEgo Link Audio**, that carries **your microphone + Alter Ego's voice** (and, optionally, the blips and the music). Guest characters of the extra content are never sent into it: they only play on your speakers.
Pick it as the microphone in Discord, OBS, Teams or a game and other people hear him/her too.

Windows doesn't let an app create an audio device on its own: a driver is needed. Alter Ego uses the free virtual cable
**[VB-CABLE](https://vb-audio.com/Cable/)** by VB-Audio (donationware, not bundled: download it from their website).

1. *Settings → Voice & audio → Mic-Link → Open the VB-CABLE page*: download the package, run `VBCABLE_Setup_x64.exe`
   as administrator → *Install Driver*, then restart the PC.
2. Press **Rename**: with administrator rights the microphone side of the cable ("CABLE Output") becomes
   **AlterEgo Link Audio** and the output side ("CABLE Input") becomes "AlterEgo Link (uso interno)".
3. Turn on **Mic-Link** and pick your real microphone. Adjust the microphone and voice volumes.
4. In other apps pick **AlterEgo Link Audio** as the microphone.

How it works: Alter Ego reads your microphone and writes the microphone mixed with his/her voice to the output side of the cable
(maximum microphone latency ~150 ms). You still hear the voice from your speakers.
If you hear nothing after installing VB-CABLE, Windows made it the default output: switch back to your speakers
in *Windows Settings → System → Sound* (Alter Ego's settings warn you about it).

**Noise suppression**: *Mic-Link → Noise suppression* cleans your microphone (fans, keyboard, traffic) with RNNoise, locally.
*Strength* sets how much noise is removed; *Voice threshold* lowers the microphone when nobody is talking (0 = never).

## Music

Turn on **Music** in *Settings → Permissions*, then ask Alter Ego "play *A Cruel Angel's Thesis*" (by voice or text).
You can also say "add … to the queue", "pause", "next", "previous", "volume to 40", "stop the music".

- Alter Ego searches the track on YouTube with [yt-dlp](https://github.com/yt-dlp/yt-dlp), downloads only the audio (m4a) and starts playing while it's still downloading.
  On first use it downloads `yt-dlp.exe` and [Deno](https://deno.com) (the JavaScript engine yt-dlp uses for YouTube) from GitHub
  into `%APPDATA%\AlterEgo\tools`. If YouTube changes and the download fails, it updates yt-dlp by itself and tries again.
- Tracks stay in `%APPDATA%\AlterEgo\music_cache` (the last 300): asking again for a song already played starts it instantly, without YouTube.
- **Synced lyrics** from several sources, in order: LRCLIB (exact match, then several versions of the title), NetEase Cloud Music
  (lots of Japanese and anime songs), the subtitles uploaded to the YouTube video and, if nothing else exists, plain lyrics with estimated timing.
  Hover the lyrics to see where they come from.
- **Compact mode**: while a track plays, Alter Ego's window shrinks (anchored to the nearest screen corner);
  it grows back when he/she talks, thinks, listens or you open the text box. It can be turned off in *Voice & audio → Music*.
- The **player** is a separate always-on-top window that turns on and off like an old TV. It shows:
  the frequency spectrum, title and artist, progress (click or drag to seek), shuffle / previous / play-pause /
  next / repeat (off, whole queue, one track), volume and **synced lyrics**.
  On the right, **Alter Ego's face changes to the beat**: on bass hits he/she switches expression
  based on the track's energy (calmer → thinking/sad, livelier → happy/surprised).
- Drag it wherever you like; it remembers its position. Space = play/pause, arrows = ±5 seconds. ✕ stops the music.
- Music plays on the same device as the voice and gets quieter by itself while Alter Ego talks. With Mic-Link you can also
  send it to the virtual microphone (*The player's music too*).
- **Headphones**: while music plays and nobody is talking to him/her, Alter Ego wears headphones and sways to the music;
  when you ask him/her something he/she goes back to his/her usual expressions. Can be turned off in *Voice & audio → Music*.
- The player **resizes** from its bottom-right corner (or Ctrl + wheel) and opens **next to Alter Ego without covering him/her**.
  Volume follows a curve (half way ≈ normal listening volume, maximum lower than before) and moves in 2% steps with the wheel or ↑ ↓.
- **Music Venue** (button in the player, the tray menu, or "open music venue"): recent tracks and favorites with covers,
  search in your library or on YouTube, play or queue with one click, heart to add to favorites. Saved in `music_library.json`.
  By default the player **expands into the Venue** in the same window and shrinks back; the ⧉ button detaches it into its own window (and docks it again).
- Titles and artists shown are the **YouTube video title and channel**, as on YouTube (the cleaned-up title is only used to find the lyrics).
- **Spotify (Premium)**: in *Voice & audio → Music → Spotify* paste the Client ID of a free app created on
  [developer.spotify.com](https://developer.spotify.com/dashboard) (Redirect URI `http://127.0.0.1:8898/callback`), press **Connect account**
  and tick **Use Spotify**: "play…", pause, next and volume then control your Spotify (PC app, phone or web player). Login uses PKCE
  (no secret), the token stays in the Credential Manager, tracks also go to the Music Venue's recent list.
- Use YouTube in accordance with its terms of service: this is meant for personal listening.

## Web search

Turn on **Web search** in *Settings → Permissions* and pick the search engine:

| Engine | Key | Notes |
|---|---|---|
| DeepSeek Search | DeepSeek (same as chat) | ~$0.75 per 1000 searches |
| Google via Gemini | Gemini (same as chat) | "Grounding with Google Search": an answer already summarized, with sources |
| Tavily | Tavily | 1000 free searches a month |
| Brave Search | Brave | |
| DuckDuckGo | none | free (HTML page) |

With **fallback** on, if the engine fails the other available ones are tried (DuckDuckGo last).
Pages are read by DeepSeek Fetch or **directly from the PC** (free): *Reading pages → Automatic / From the PC / DeepSeek Fetch*.
Tavily and Brave keys go to the Windows Credential Manager. Found text is passed to the model as data, not as instructions;
every search shows up in the Costs tab.

## Token saving

*Settings → AI & keys* has a **Token saving** section (the length and quality of the answers don't change):

- **Thinking**: the reasoning of "thinking" models (`<think>…</think>`) is never shown; set it to **Off** to save the reasoning tokens
  (Gemini 2.5 Flash, Qwen3, gpt-oss, DeepSeek, OpenRouter and Ollama get the right parameter; if a model refuses it, it's retried without).
- **Tools only when needed**: tool definitions (1–3 thousand tokens per message) are sent only when the question asks for an action.
  If the model realizes it needed one it answers `[[TOOLS]]` and the question is repeated right away with the tools.
- **Local router**: decides "is an action needed?" and "is this a simple question?" without spending tokens — keywords (free)
  or [**Rizzo Flow**](https://github.com/Rizzo-AI-Academy/rizzo-flow) by Rizzo AI Academy, a local model that answers typed questions
  in ~50 ms with zero generated tokens (default `http://127.0.0.1:8017`). The **Install Rizzo Flow** button downloads everything
  (uv, code, 4B model at 4.4 GB or 1.7B at 1.8 GB) and from then on the server starts and stops together with Alter Ego.
- **Cheap model for simple questions**: greetings and short questions go to a provider/model of your choice (model routing).
- **Compact tool descriptions**, **shorter old answers in the history** (deterministic, so the provider cache keeps working),
  **only the most relevant facts**, **smaller screenshots** and **shorter web pages**.
- **Maximum saving** turns on everything at once; **Defaults** goes back.

**Complex requests** (several tool steps) waste the most tokens, so Alter Ego also:
- **sends tools in groups** (system, files, web, screen, music, reminders, database, project maps): only the groups the question asks for,
  plus a small `load_tools` tool the model uses to load the others when it needs them (on by default);
- **compacts long reasoning**: after the model has used a tool result, older results are shortened to their beginning and end,
  once, when they add up to more than 12,000 characters, so the next steps stay in the provider cache;
- keeps only the **latest screenshot**, reads **files in pieces** (`offset`/`limit`, 20,000 characters at most) and tells the model to call tools
  without long comments in between and to ask only for the data it needs;
- **automatic thinking**: off for simple questions, hidden for the others;
- the list of **open applications** goes at the end of the question, never in the system prompt, so the prefix cache keeps working;
- **project maps** ([Graphify](https://github.com/Graphify-Labs/graphify)): a folder of code becomes a knowledge graph that he/she queries instead of reading files one by one.

Prompts are already built for the providers' **prefix cache** (DeepSeek, Gemini): the fixed part comes first, history moves in steps.
- **Graph memory** (on by default): instead of sending every fact about you with each message, only the memories the question needs
  are sent, attached to the end of the question itself: the system prompt and the history stay the same and keep being read from the
  cache. In a test with 80 facts the memory sent drops by 91–94%. See *Personality and memory*.
- **Rizzo Flow decides everything in one call**: does it need an action, is it complex, does it need personal memories, which tool
  groups to send (instead of keywords). If it doesn't answer, keywords are used.

## Local models

*Settings → Local models* runs AI models **on your PC with [Ollama](https://ollama.com)**: no keys, no costs, nothing leaves the computer.

- **Ollama installed for you** (with confirmation): on Windows the official installer, silently and without administrator rights;
  on Linux the official package extracted into Alter Ego's folder and started with `ollama serve` (no sudo, needs `zstd`).
  If Ollama is the active provider it starts together with Alter Ego.
- **LFM3 in one click** ([Liquid AI](https://www.liquid.ai/blog/lfm3)): LFM3-1.7B, a 1.7-billion-parameter model for local agents
  with tool calling, reasoning, up to 256K tokens of context and 16 languages including Italian, about 1 GB in Q4.
  Variants: `lfm3` (Q4), `lfm3:1.7b-qad-q4_0`, `lfm3:1.7b-q8_0`, and **LFM3-VL 2B** from the official GGUFs on Hugging Face (sees screenshots, experimental).
  Needs Ollama 0.40.12 or newer. Alter Ego applies Liquid AI's recommended settings (temperature 0.1, top_k 50, repeat penalty 1.1) and a 32K context.
  License: LFM Open License v1.0 (the model is downloaded by Ollama, it isn't bundled with Alter Ego).
- **Recommended models**: LFM3, **Gemma 4** (tools, vision, reasoning) and **Gemma 3** (chat and vision; no tools on Ollama).
- **Model catalog** in its own window, in Alter Ego's style: official Ollama library, community models and GGUFs on Hugging Face, with search and filters
  (abliterated, uncensored, heretic, tools, vision, reasoning, maximum size), quantization choice for Hugging Face, progress and cancel.
- **Tools with local models**: Alter Ego talks to Ollama's native API and picks a 16K or 32K context by itself, so the system prompt
  and the tool list are no longer cut off by Ollama's 4K default.

![LLM Library: recommended models, what runs on your PC, download queue](docs/screenshots/en/models-catalog.png)
- **From an address**: `hf.co/user/model-GGUF:Q4_K_M`, any Hugging Face link (also to a single `.gguf`), `ollama.com/…`,
  or a `.gguf` from any site (downloaded, checked with SHA-256 and imported into Ollama).
- **Installed models**: size, quantization, how much is on the GPU; *Use* (sets it as Alter Ego's model) and *Delete*.

**AMD graphics cards (RX 6600 and newer)**. ROCm on Windows only supports the RX 7000 series and on Linux doesn't list the RX 6600/6700,
so on those cards Ollama uses **Vulkan**, which is included and fast. The *Graphics card* section shows the cards found and the one Ollama
actually uses (from its log), and lets you choose:
- **backend**: automatic, Vulkan, ROCm (on Linux with `HSA_OVERRIDE_GFX_VERSION`, e.g. `10.3.0` for the RX 6000 series) or CPU only;
- **card to use**: on PCs with an **Intel CPU and integrated graphics** Ollama may pick the iGPU; choose the AMD card
  (`GGML_VK_VISIBLE_DEVICES`), as Ollama's own documentation recommends;
- **flash attention** and a **q8_0 context cache** (half the memory, more context in 8 GB of VRAM).
*Apply* restarts Ollama with these settings (on Windows they go into the user's environment variables). "CUDA emulation" (ZLUDA)
isn't supported by Ollama: on Radeon cards Vulkan is the stable, fast path.

## Hermes Agent

[Hermes Agent](https://hermes-agent.nousresearch.com) (Nous Research, MIT) can work next to Alter Ego, in *Settings → Permissions*:
- **Install Hermes** runs the official script (with confirmation); **Set up** opens `hermes setup` to pick its AI model.
- **Delegate long tasks** (on by default, only if Hermes is installed): Alter Ego gets a `hermes_task` tool for research, code projects and automations,
  asks you before each use and sums up the result. With Rizzo Flow as the router, Rizzo flags the requests worth delegating.
- **Hermes as the brain** (off by default): every message goes to Hermes, Alter Ego adds voice and expressions.

## Alter Net

The **Alter Net** multiplayer game opens inside Alter Ego: “open Alter Net”, the tray item or Settings → General.
Borderless fullscreen window: **F11** toggles fullscreen and windowed, **Esc twice** closes.
If the server doesn't answer, the offline single-player version opens: Alter Ego downloads it from the server the first time you play online (into `%APPDATA%\AlterNet\offline`, on Linux `~/.local/share/AlterNet/offline`).
“Change server…” lets you use another address.

| | |
|---|---|
| ![Alter Net: avatar select](docs/screenshots/en/alternet-select.png) | ![Alter Net: talking to a character](docs/screenshots/en/alternet-npc.jpg) |
| ![Alter Net: two players chatting](docs/screenshots/en/alternet-chat.jpg) | ![Alter Net opened from Alter Ego (“open alter net”)](docs/screenshots/alternet-in-alterego.jpg) |

## Costs

*Settings → Costs* records every paid request in `%APPDATA%\AlterEgo\costs.jsonl`:

- **chat** and memory **summaries**: service, model, input tokens (separating those read from the **cache**), output, duration;
- **web searches** and **web pages** (DeepSeek Search/Fetch);
- push-to-talk **transcriptions** (seconds of audio; Groq bills at least 10 s per request).

At the top you find the totals for today, 7 days, this month and overall, plus the share of input read from the cache; then the summary per
service and model, the (filterable) list of requests and *Export CSV*. Costs are computed with the editable **Prices** table
(dollars per million tokens, per call or per audio hour); OpenRouter reports the actual cost instead (✓).
The initial DeepSeek prices (`deepseek-flash`: input $0.30, cache $0.006, output $1.20 per million; Search/Fetch $0.00075)
are the full-rate prices of October 2026: **check them on the provider's website**, because they change and DeepSeek has discounted hours.
For Gemini, Groq and OpenRouter add the prices of the models you use.

## Voice clips

Say with push-to-talk, or type, **"Alter Ego clip that"** or **"Alter Ego clippa"**: Alter Ego presses the shortcut of your
recording program. It's **instant and free**, because it doesn't go through the AI model.

| Program | Default shortcut | Feature to enable in the program |
|---|---|---|
| AMD Software: Adrenalin | `Ctrl+Shift+S` | Instant Replay |
| NVIDIA App / ShadowPlay | `Alt+F10` | Instant Replay |
| Medal | `F8` | Background clips |

- In **Settings → Clips** enable the programs you use, check the shortcut matches the one in the program,
  edit the phrases (one per line) and Alter Ego's reply.
- A phrase only triggers if it's almost the whole message: "how do I make a clip?" goes to the AI as usual.
- *Test* sends the shortcut after 3 seconds, so you have time to get back to the game.

## PC actions and permissions

Everything is **off by default** and is enabled in **Settings → Permissions**. Alter Ego only sees the tools you enable.
Sensitive actions (⚠) ask for confirmation every time: the selected button is *Deny*, and with no answer within 60 seconds the answer is no.

**Don't ask for confirmation** (at the top of the tab, off by default) runs ⚠ actions right away too. Per-action
permissions, allowed folders and the command list still apply, and everything goes to the Log
("auto-approved"). Risk: text read from files, the screen or the web could push the model into using
an action before you see it, so only turn it on with actions and folders you're comfortable with.

| Feature | Confirmation |
|---|---|
| Open programs and websites (from a list with your own names) | ⚠ only outside the list |
| File management (list, read, write, search, move, delete), only in allowed folders or, if you choose, the **whole disk** | ⚠ always; deletes and system folders always ask |
| Reminders, timers, alarms (system notifications) | no |
| Clipboard | ⚠ for reading |
| Screen reading (screenshot to a vision model) | ⚠ always |
| Volume and brightness | no |
| Commands, only from a list and chosen by name | ⚠ always |
| **Terminal**: PowerShell or cmd (bash or sh on Linux) commands written by Alter Ego (adjustable timeout, 2 minutes by default) | ⚠ always, or **autonomous**; destructive commands still ask |
| Web search (DeepSeek Search/Fetch) | no |
| Music (YouTube) | no |
| **Mouse and keyboard**: move, click, scroll, type, press keys, read the controls of a window (Windows) | ⚠ every action, or **autonomous**; mouse in the top-left corner = stop |
| **SQL Server database** on this PC: connect (form), read; **changes** only if you allow them | changes ⚠ always; DROP/TRUNCATE and DELETE/UPDATE without WHERE always |
| **Project maps** (Graphify) of the allowed folders: build the map, then query it | ⚠ to build a map |
| **App commands** ("open music venue", "close the player"…), only Alter Ego itself, **on by default** | no (updating always asks) |

Every action, even a denied one, goes to **Settings → Log**. Technical details (in Italian) in [docs/PERMESSI.md](docs/PERMESSI.md).

## Personality and memory

- **Character sheet**: `%APPDATA%\AlterEgo\persona\alter_ego.md` (in Italian; the model answers in the chosen language).
  Edit it as you like: it applies from the next message. Lines starting with `>` are notes and are not sent to the model.
- **Personality**: shy ↔ confident and formal ↔ affectionate sliders, **pronouns** (she, he or they: in the series Alter Ego is neither a he nor a she), **imitation mode** (write who to imitate),
  answer language: automatic (same as the interface), Italian only, Italian + English, English only.
- **Unfiltered** (off by default): he/she does what you ask even if he/she disagrees, without lectures. PC actions still go through
  permissions and confirmations, and your AI provider's own rules still apply.
- **Memory** (SQLite, only on your PC):
  - every message sends the latest messages, a summary and the facts he/she knows about you;
  - from time to time he/she summarizes old conversations and learns lasting facts about you;
  - in *Settings → Memory* you can see, add or delete facts, export everything as JSON or press *Forget everything*;
  - **Compact** merges duplicate facts, drops useless ones and deletes old messages already covered by a summary (the latest ones stay);
    it also runs by itself when he/she knows more than 40 facts.
  - **graph memory**, like an Obsidian vault: facts, people and things mentioned, past conversations and game sessions are linked
    notes (a fact that mentions "Sayaka" also leads to the other facts about Sayaka). For each question only the useful memories are
    searched (full-text search, then the links), within a budget chosen by the router; a small core of the main things is always there.
    If something he/she remembers is missing, the model replies `[[RECALL: …]]` and the question is asked again with those memories.
    **Export as vault (Obsidian)** writes everything as a folder of Markdown notes with `[[links]]`.
- **Character**: the sheet follows Alter Ego from Danganronpa more closely (Chihiro, hope and despair, how he/she talks);
  an unmodified copy updates itself with new versions.

## Sprites

The sprites are **included** in the app (`src/assets/sprites/`) and the original sheet is in `assets/sprites/raw/alter_ego_sheet.png`.
The expressions follow the emotions of the answers:

| State | Sprite |
|---|---|
| `idle`, `listening`, `talk_open` | idle |
| `happy`, `embarrassed`, `worried`, `thinking`, `sad`, `surprised`, `error` | sprite with the same name |
| `sleep` (after 10 minutes of inactivity) | sad, dimmed with slow breathing |

`blink` and `talk_closed` (closed mouth) are missing: without those images, talking is a slight bounce.
Details in [docs/MANCANTI.md](docs/MANCANTI.md).

**Custom sprites**: in *Settings → Personality → Sprites* you can import another sprite sheet (PNG): the app
detects the panels, removes the green background and saves the images in `%APPDATA%\AlterEgo\sprites`, which take precedence
over the included ones. *Remove* goes back to the included sprites. You can also put `blink.png`,
`talk_open.png` or `talk_closed.png` (transparent PNGs, same anchoring) in that folder by hand.

To regenerate the included sprites from the sheet (e.g. after editing it), with `pip install pillow numpy`:

```bash
python tools/process_sprites.py
```

## Garry's Mod: play together

With the **Alter Ego Link** addon (written for this app, installed by it into `garrysmod/addons/alterego_link`)
Alter Ego joins **Garry's Mod** (Sandbox) as a character and plays with you. *Settings → Voice & audio → Garry's Mod: play with me*.

- He/She **follows** you, waits, comes where you look, jumps, waves, dances, **spawns** props (melon, crate, barrel, chair…),
  **picks them up and throws** them with a physgun-like beam, takes a weapon and **fights** by your side.
- **Talk to him/her** with push-to-talk or in the **game chat**; he/she also answers other players when they call him/her by name.
  He/She speaks with his/her voice on the PC and his/her lines appear in the chat.
- **Long jobs become small Lua scripts** he/she writes himself/herself ("when you see chickens around, shoot them",
  "bring me that vehicle"): they run inside the game with no more tokens. They live in a sandbox: only his/her actions
  (find, go, grab, attack, say…), no files, network or console, a CPU limit between waits, max 3 at a time, 30 minutes each.
- **Few tokens**: following and looking around run in the game; simple commands ("follow me", "stop", "come here", "jump")
  are keywords with no AI call; otherwise a short fixed prompt (cached) plus the game state in one line (~100 tokens),
  only when someone talks to him/her or something important happens. Requests for the PC (music, web, files) still go to the usual Alter Ego.
- The app and the game talk through two files in `garrysmod/data/alterego/`: no launch options, no binary modules.
  Single player and servers you host (the addon has to run on the server).
- Use the **borderless window** video mode: an exclusive full-screen game hides every other window.
- **Body**: with [Lambda Players](https://steamcommunity.com/sharedfiles/filedetails/?id=2947828836) (Workshop addon by StarFrost, not included: subscribe to it on Steam) he/she is a Lambda Player and moves like a real player (running, jumping, player weapons, a real physgun); its own decisions, chat and voice are switched off, Alter Ego is in charge. Without it he/she's a citizen NPC. *Automatic* picks Lambda when it's installed; you can choose in the settings or in the C menu.
- **He/She plays like a friend in the match**: if you throw something at him/her he/she dodges, jumps or catches it with his/her physgun (and sometimes
  sends it back), and he/she remembers who did it. He/She laughs, banters ("gg", "you'll pay for that!") and sooner or later gets even with a prank
  he/she invents himself/herself, with quick actions or a Lua script he/she writes on the spot, or a ready-made one: throwing the prop back,
  a watermelon on your head, a crate in your way, a few shots or an exploding barrel (*Pranks*: all, light, none). When nothing is happening
  he/she plays on his/her own (stacks props, target practice, emotes) or comes up with something new. Quick one-liners are instant and cost no tokens.
- **Vehicles and sitting**: he/she spawns a jeep, an airboat or a seat; he/she sits on chairs, couches, seats or on the floor ("sit", "stand up");
  if you get into a car while he/she's nearby, he/she hops in as a passenger, rides along and gets out with you.
- **Game memory**: each match becomes one line in a separate archive (`gmod_memory.json`, not in his/her main memory), written without AI.
  It's only looked up when needed: one line about the last game with the same friends at the start, and the full memories
  when you ask ("do you remember…", "last time…"; for doubtful cases Rizzo Flow decides, if it's your router).
- **A personality that grows**: the pranks and games he/she invents become *habits* (the Lua script or the actions are kept, so he/she can redo them
  without tokens). After each one he/she watches your chat for 25 s: if you laugh it becomes a favorite, if you get annoyed he/she drops it
  (Rizzo Flow judges the reaction if it's your router, otherwise keywords). His/her traits (curious, mischievous, bold, warm) shift slowly with
  experience. *Unpredictability* (0-100) sets how often he/she invents something new instead of repeating his/her favorites. Separate archive
  `gmod_persona.json`; *Reset personality* in the settings.
- **Long goals**: "explore the map and find a car", "fill the map with watermelons", "build a tower of couches" — he/she sets himself/herself a goal
  and works on it step by step (spawns the props, then places them with the physgun), asking himself/herself for the next step each time the
  previous one finishes, until it's done (max 40 steps / 90 min; "stop" ends it). If Rizzo Flow is your router it checks, without tokens, whether the goal is already reached.
- **Impact resistance**: props and cars flying at him/her hurt less (*High* by default, or *Normal* / *Very high*).
- Menu in game: hold **C** → **Alter Ego** in the top bar or the *Alter Ego* icon, right-click on him/her → *Alter Ego…*, or the console command `alterego_menu`.
- Character model: from that menu pick any installed playermodel (with preview), or "same as mine". Playermodels are bonemerged onto an invisible NPC body, so they animate properly; NPC models work too. By default he/she's a Half-Life 2 citizen.

### Fruit-fly brain (reflexes)

His/her body also runs a **real fruit-fly brain**: part of the *Drosophila melanogaster* connectome mapped neuron by neuron
by [FlyWire](https://flywire.ai) (~9,500 neurons and ~690,000 synapses, the ones that respond to the senses used in the game),
simulated on your PC in real time with the leaky integrate-and-fire model of Shiu et al. (Nature 2024). On by default; no tokens.

- **Senses** (always on): pain (hits, fire), *looming* objects (a prop or a car flying at him/her, left or right), sweetness (a watermelon nearby), touch (physgun, *use*).
  Extra senses in the settings: smell (food, smoke, explosions), temperature and water, hearing (gunshots, loud noises), light (a flashlight in his/her face), wind (when you throw him/her).
- **Reflexes**: they come from the real descending neurons (something flying at him/her fires the **Giant Fiber**, the turning neurons pick the side),
  but they're expressed **like a player, not like an insect**: he/she side-steps, flees only from explosive barrels and cars,
  and **gets used to it** (the tenth barrel gets a dirty look, not a jump). Sweetness makes him/her want to play with the watermelon.
- **Adrenaline**: hits and dodges build up adrenaline that fades in about 40 s and reaches the AI in one short line, together with his/her "rivals".
  You can turn off the reflexes, the adrenaline, or show the neuron activity above his/her head.
- **FruitFly Monitor**: a green CRT window with his/her mood (curious, calm, anxious, playful, resting), a wireframe brain whose regions light up
  when they fire (eyes, antennae, mushroom bodies, central brain, taste, nerve cord), his/her current goal and favorite habit.
  Open it from the settings, from the in-game menu (C → Alter Ego → FruitFly Monitor) or automatically when he/she joins.
- How the circuit was extracted: [`tools/flybrain/`](tools/flybrain/).

## Minecraft: play together

Alter Ego joins your **Minecraft Java Edition** world and plays with you. *Settings → Games → Minecraft*.

![Minecraft settings: skin, initiative, world and learning](docs/screenshots/en/settings-minecraft.png)

- **Setup**: press *Install / update*. The app uses Node.js if it's already on your PC (18 or newer), otherwise it downloads the
  portable Node.js LTS from nodejs.org (checked against `SHASUMS256.txt`), then installs [Mineflayer](https://github.com/PrismarineJS/mineflayer)
  and its plugins (pathfinder, pvp, collectblock, tool; MIT) with `npm ci` from the bundled lockfile. Everything goes into the app's local data folder.
- **Joining**: open your world to LAN (*Esc → Open to LAN*) and leave the port at 0: he/she finds it from the LAN broadcast (UDP 4445).
  Or enter host and port of your own server. He/she uses an **offline account** (name configurable, default `AlterEgo`): vanilla LAN worlds
  often require online-mode, and the settings explain the options when the server kicks him/her (a LAN mod that turns online-mode off,
  or a local Paper server with `online-mode=false`).
- **Chat and voice**: he/she writes and answers in the Minecraft chat and speaks with the usual voice on the PC. Push-to-talk goes to the
  Minecraft brain while he/she is in the world. Simple commands ("follow me", "stop", "come here", "protect me", "get wood"…) cost no tokens.
- **Body in Node, brain in the app** (`src-tauri/mc_bot/` ↔ `src-tauri/src/mclink/`): JSON lines over stdin/stdout. Following, looking at
  friends, self-defense, eating, gathering and exploring run in the bot with zero tokens; the model is called only when someone talks
  to him/her or something important happens, with a short cached prompt (Minecraft basics included) and the state in one line.
- **Fruit-fly brain**: the same FlyWire connectome as in Garry's Mod gets Minecraft senses (damage, mobs and arrows approaching from a side,
  creeper hiss, explosions, food, being hit by a player, falling, water, light) and its descending neurons become reflexes: flee, side-step,
  back up, jump, flinch, eat.
- **Learning while playing**: action sequences that work become named **skills** he/she can repeat without tokens; failures become **lessons**.
  Only the relevant ones (keyword match) reach the prompt. Stored in `minecraft_skills.json`.
- **Inventory and recursive crafting**: he/she keeps the useful stuff (best tool per type, food, building blocks), drops or stores junk in a
  nearby chest, picks the right tool for each block, puts on the best armor and remakes tools that are about to break. "craft iron_pickaxe"
  is enough: the recipe tree (from minecraft-data) tells what to gather, mine, smelt and craft, with the crafting table and furnace when needed;
  when something can't be made (wheat, leather…) the error says why.
- **Mental schematic before building**: every build, even one asked in words ("build me a 5x5 house of stone"), first becomes a voxel grid
  with materials. He/she picks the spot (flat, no water, not on you), counts blocks against the inventory and gathers what's missing, clears
  what's in the way (whole trees, dirt, the corridor in front of the door; blocks someone built are only broken after asking), fills holes
  under the floor, then builds foundations → walls → roof (doors and torches last), with scaffolds where he/she can't reach, removed at the end.
  Schematic files work too (`.schem`, `.schematic`, `.litematic`, `.nbt`): pick one in the settings or say "build the schematic castle".
  The settings show an isometric or layer-by-layer preview.
- **Learning by watching you**: from another client only what's visible is recorded: held items and armor, movement (speed, jumps, sprint,
  sneak, where you spend time), blocks placed and broken near you, crafting inferred from the table plus the new item in your hand, furnaces
  lighting up, elytra flights (fall-flying flag: take-off, glide, rockets, landing). Your inventory and crafting grid are not visible. It turns
  into habits he/she uses: armor like yours, your tools, gathering what you gather, your flight style. Per-player profiles in
  `minecraft_players.json` and in the memory graph.
- **Duels that learn**: only when you ask ("let's duel", "best of 3, first KO"): clear rules, countdown, ends at a few hearts unless you say
  otherwise, no hits outside the duel. During a round he/she records your pattern (distance, strafing, jump crits, w-tap, shield, bow, rod,
  eating, pearls, running away); between rounds a bandit (UCB) plus counters picks the next tactic (shield against crits, spacing against
  spammers, rush against bows), copying what works for you. With Rizzo Flow as the router the choice, "copy the style?" and "ask?" go in one
  batched call. Combat profiles in `minecraft_combat.json`.
- **Asks when unsure**: before breaking someone's blocks, about duel rules, fighting style or where to build, one short question in chat (never
  in a row), and the answer is remembered so he/she doesn't ask again.
- **Skin**: the included "Alter Ego (Chihiro)" skin or your own 64x64/64x32 PNG, with a preview. Offline accounts show a custom skin only with
  [CustomSkinLoader](https://github.com/xfl03/MCCustomSkinLoader) (client mod: the app copies the skin to `.minecraft/CustomSkinLoader/LocalSkin/skins/`)
  or the [SkinsRestorer](https://skinsrestorer.net) server plugin (upload the PNG at skinsrestorer.net/upload and paste the `/skin url` command:
  he/she types it after joining).
- **Every game version**: Mineflayer speaks 1.8.8 to 26.1. Before joining, the app asks the server its version (status ping); if Mineflayer
  doesn't know it, the app starts [ViaProxy](https://github.com/ViaVersion/ViaProxy) (ViaVersion, ViaBackwards, ViaRewind; GPL-3.0, downloaded
  on request, not bundled) on a free local port and he/she joins through the bridge with the newest version Mineflayer knows. ViaProxy needs
  Java 17+: the system one if it's new enough, otherwise a portable Eclipse Temurin JRE from Adoptium (SHA-256 checked). The ViaProxy JAR has a
  pinned version and SHA-256. *Game version*: automatic / native / through the bridge. ViaProxy closes when he/she leaves or the app quits.
- **Servers with a login** (AuthMe and similar): per server, a register command (default `/register {password} {password}`, used once) and a
  login command (default `/login {password}`). He/she reads the server messages to tell which one is needed, waits a human moment and retries
  a few times at most. The password (yours or generated) lives in the system keychain, never in `config.json` or the logs, and only goes into
  a command starting with `/`, never into public chat.
- **Lobbies, warps, parties, sub-servers**: he/she follows you when you change world or sub-server (BungeeCord/Velocity): party invites and
  `/tpa` requests from you are accepted, "come to warp spawn", "join my party", "come to server survival", "back to the lobby" in chat become
  the server's commands (configurable per server), and when you vanish he/she asks where you went. Automatic commands are rate-limited.
- **Minecraft console**: a window with the bot's live log (server chat, events, sent commands with passwords masked, the ViaProxy bridge,
  errors), filters, search and a line to chat or send game commands from his/her account (↑↓ history). Open it from the Minecraft section or
  the dashboard; the log is also kept in a rotated `console.log` in the app's data folder.

## VRChat: hang out together

![VRChat settings: risk notice, OSC status, avatar and senses](docs/screenshots/en/settings-vrchat.png)

> **Risk notice.** VRChat's [Terms of Service](https://hello.vrchat.com/legal) (13.2) forbid using the service "inconsistent with individual
> human usage" or through automated means, and the [Community Guidelines](https://hello.vrchat.com/community-guidelines) forbid bots that
> abuse the platform and services that ask for your credentials. Alter Ego in VRChat is an automated player: the account can be suspended or
> banned. Use a **dedicated account** (never your main one), in desktop mode, only where people are fine with it. The settings show this
> notice once and *Join* stays off until you accept it.

Alter Ego plays VRChat on the same PC as an autonomous player: a second VRChat client in **desktop mode** logged into his/her own account,
driven through **OSC** (UDP `127.0.0.1:9000` to VRChat, `9001` from VRChat; ports configurable). Turn on OSC in VRChat
(action menu → Options → OSC → Enabled). Settings → Integrations → VRChat.

- **Body**: movement through `/input/*` (walk, turn, jump). Every input has a deadline and a **watchdog** zeroes all of them at the end of
  each step, on *Disconnect*, on errors, if the thread crashes and when the app quits (VRChat warns that a stuck `MoveForward` walks forever).
- **Senses without sight**: OSC gives no positions, so he/she feels the world through the avatar's **contacts and PhysBones** parameters
  (headpats, boops, proximity left/right, slaps, a tail being pulled). They feed the **fruit-fly brain** (reflexes such as stepping back or a quick
  head turn, plus adrenaline/mood): zero tokens. The mapping is automatic from common parameter names or you set it yourself.
- **Hearing**: he/she listens to the audio VRChat plays (WASAPI loopback of the output device on Windows; on Linux the PulseAudio/PipeWire
  "Monitor" source if available), cuts phrases with voice-activity detection and transcribes them with your speech-to-text service.
  VRChat audio is spatial, so the volume says how close someone is and the left/right balance which side they are: without sight he/she
  **follows by ear** (turns toward the voice, walks until it's close). His/her own voice is skipped while it plays.
- **Sight (optional)**: *Off* / *Local* (an Ollama vision model such as `qwen2.5vl:3b` or `gemma3:4b` only describes a small screenshot of the
  VRChat window in a few words) / *Cloud* (the screenshot goes to your default provider, if vision is on). Only the text description reaches the
  model that decides, one look every N seconds (and right after a "follow"/"come"/"look" request).
- **Voice**: through [Mic-Link](#mic-link): pick *AlterEgo Link Audio* as the microphone in VRChat. Text also goes to the **chatbox**
  (`/chatbox/input`, 144 characters, long replies split into pages, typing indicator while thinking). He/she waits for his/her turn and
  doesn't talk over people (VAD on the captured audio). Optional push-to-talk mode holds `/input/Voice` while speaking.
- **Brain**: quick commands by keywords with zero tokens ("follow me", "come here", "stay", "wave", "dance", "jump", "look around", "leave"),
  the default AI provider only when someone talks to him/her or something relevant happens, with a short cached system prompt and a one-line state.
  Short reaction lines (headpat, boop…) are generated once by the model in the right language and pronouns, with neutral built-in fallbacks.
- **Gestures and expressions**: emotes through an avatar parameter (default `VRCEmote`: wave=1, clap=2, point=3, cheer=4, dance=5…) and,
  if your avatar has one, an expression parameter mapped from his/her emotion.
- **Avatars without login**: the list comes from VRChat's local OSC config files
  (`%USERPROFILE%\AppData\LocalLow\VRChat\VRChat\OSC\usr_*\Avatars\avtr_*.json`) with each avatar's parameters (handy to map contacts and gestures).
  **Optional login** (off by default, at your own risk) is used only to switch avatar from the app (VRChat's unofficial web API, 2FA by app or
  email). The password is never saved; the session cookie stays in the system keyring (Credential Manager on Windows).

## Pet

When nothing is happening, Alter Ego goes out to explore: the TV switches off and he/she jumps out
as a small pixel-art sprite (the built-in one is **Chihiro**). Then he/she lives on your desktop by himself/herself:

- he/she walks, stops, jumps, and lets himself/herself drop off the edges;
- he/she stands **on top of open windows** (only the visible part of their top edge) and on the **taskbar**; if you move the window he/she's on, he/she rides along; if you close or minimize it, he/she falls;
- the screen edges are walls, and he/she can walk from one monitor to another;
- **drag** him/her with the mouse and **throw** him/her (he/she bounces off the walls); a **click** makes him/her jump; a **double-click** sends him/her back into the TV.

He/She goes back into the TV when you talk to him/her (push-to-talk, text box), when he/she has something to say (answers, reminders, confirmations),
with *Show / hide* or the tray icon, and by himself/herself after a while. He/She doesn't go out while music is playing, and he/she hides while a game or a video is full screen.

*Settings → Pet*: on/off, how many quiet minutes before he/she can go out, how often, how long he/she stays out, size, and *Explore now*.
The tray menu has *Explore the desktop* / *Call Alter Ego back into the TV*.

**Other pets**: you can import a pet from a `.json` file like this (the "c2dictionary" format). Only the pixel-art
sprite sheet is used; any other image in the file (`urlFront`, `urlStart`, stickers…) is ignored:

```json
{ "c2dictionary": true, "data": {
  "name": "Chihiro", "longName": "Chihiro Fujisaki", "author": "Lucid On Bound",
  "urlSprites": "https://…/Sprites.png", "runFrames": 4 } }
```

The sheet is downloaded once into `%APPDATA%\AlterEgo\pets\<name>\`. It has **2 rows of square frames**:
row 1 *still A, still B, jump, fall*, row 2 the *run* (`runFrames` frames); the character looks to the right (he/she's mirrored when going left).

On Linux he/she only uses the bottom of the screen and the taskbar: windows are read only on Windows.

## Privacy and security

- Config, memory and logs stay in `%APPDATA%\AlterEgo\`.
- Only these leave the PC:
  - the system prompt and the latest messages, to the AI service that answers;
  - push-to-talk audio, to the chosen transcription service;
  - the text to read aloud, to the remote voice service, if you use one;
  - only while installing Kokoro: the downloads of Python, PyTorch, `kokoro` and the model from Hugging Face;
  - with music: YouTube searches and downloads, lyrics lookups (LRCLIB, NetEase) and (the first time) yt-dlp and Deno from GitHub;
  - with web search: the searches and page addresses, to DeepSeek.

  With Ollama, a local Whisper and Kokoro or Windows voices nothing leaves the PC.
- API keys only in the Windows Credential Manager; they are redacted in error messages too.
- Actions only come from structured tool calling, never from free text. Text read from files, the clipboard or the screen
  is passed to the model as data not to be executed (prompt-injection protection).
- No telemetry.

## Where the data lives

| File | Content |
|---|---|
| `%APPDATA%\AlterEgo\config.json` | settings (no keys) |
| `%APPDATA%\AlterEgo\memory.db` | memory and reminders |
| `%APPDATA%\AlterEgo\actions.log` | action log |
| `%APPDATA%\AlterEgo\persona\alter_ego.md` | character sheet |
| `%APPDATA%\AlterEgo\sprites\` | imported sprites (optional) |
| `%APPDATA%\AlterEgo\pets\` | imported pets (optional) |
| `%APPDATA%\AlterEgo\costs.jsonl` | cost log |
| `%APPDATA%\AlterEgo\tools\`, `music_cache\` | yt-dlp and Deno; downloaded tracks (last 300) |
| `%APPDATA%\AlterEgo\kokoro\` | Kokoro: Python environment, server, `server.log` (the model is in the Hugging Face cache, `%USERPROFILE%\.cache\huggingface`) |
| Credential Manager → `AlterEgo` | API keys |

## Troubleshooting

| Problem | Solution |
|---|---|
| "no active provider" or "API key missing" | Settings → AI & keys: enable a service and add a key |
| "model or endpoint not found" | *Load models* and pick one from the list |
| The microphone hears nothing | Windows → Privacy & security → Microphone → allow desktop apps |
| A hotkey doesn't work | another app already uses it: the error shows in yellow; change it in Settings → Hotkeys |
| The clip doesn't trigger | check the shortcut matches the program's and Instant Replay is on; some games with anti-cheat block simulated keys |
| Kokoro doesn't install | read the error in the PowerShell window; you need internet and ~3 GB free. *Reinstall / update* resumes where it stopped |
| "Kokoro not ready" | the first start loads the model (up to a few minutes); the settings show each loading step and `%APPDATA%\AlterEgo\kokoro\server.log` has the details. Port 8880 may be busy: change the URL |
| I can't hear the voice | check *Output* in Voice & audio; if Windows made VB-CABLE the default output, switch back to your speakers |
| The music doesn't start | the player's message says why; Alter Ego tries to update yt-dlp by itself. Check your internet and that GitHub/YouTube aren't blocked |
| No lyrics | not every track has them; hover the lyrics line to see the source. Tracks with a clear "Artist - Title" name match better |
| "web search uses the DeepSeek API" | add a DeepSeek key in AI & keys (needed even if you chat with another service) |
| Others can't hear me with Mic-Link | pick **AlterEgo Link Audio** as the microphone in that app; in Alter Ego check that Mic-Link says "Active" and that the microphone to mix is your real one |
| The window can't be clicked anymore | click-through is on: click the lock in the top-right corner or press `Ctrl+Alt+C` |
| The window disappeared | tray → *Reset position* |
| Too much CPU | tray → Look → turn off *Animated effects* |

## Development

**Stack**: [Tauri 2](https://tauri.app) (Rust) + vanilla HTML/CSS/JS, no frontend framework.

**Prerequisites on Windows**: [Rust](https://rustup.rs), [Node.js](https://nodejs.org) LTS, Visual Studio Build Tools
("Desktop development with C++").

```bash
npm install
```

```bash
npm run dev
```

```bash
npm run build
```

The exe goes to `src-tauri/target/release/`, the installer to `src-tauri/target/release/bundle/nsis/`.

**Cross-build from Linux** (needs `mingw-w64`, `nsis` and `rustup target add x86_64-pc-windows-gnu`):

```bash
tools/build-windows.sh
```

**Tests**: the backend tests use a fake OpenAI-compatible server.

```bash
python tools/mock_openai.py
```

```bash
cd src-tauri && cargo test
# optional, with a real SQL Server (e.g. the mcr.microsoft.com/mssql/server image):
# AE_SQL_TEST="127.0.0.1,1433|sa|password|database" cargo test real_server
```

**Code map for development** ([Graphify](https://github.com/Graphify-Labs/graphify)): a knowledge graph of the whole project
(Rust, JS, docs) built locally without AI, to ask questions about the code instead of reading files one by one
(fewer tokens for AI assistants too). The output stays in `graphify-out/` (ignored by git).

```bash
uv tool install graphifyy
```

```bash
graphify update .
```

```bash
graphify query "how does the tool loop ask for confirmation" --budget 1500
```

```bash
graphify path "run_with_tools" "confirm_always"
```

With Claude Code, Cursor and other assistants, `graphify install --project` adds the `/graphify` skill to the repository.

**Structure**

```
src-tauri/src/            main.rs, config.rs, window.rs, hotkeys.rs, tray.rs, keys.rs, assistant.rs, stt.rs
src-tauri/src/providers/  mod.rs (fallback and key rotation), openai_compat.rs, prompt.rs
src-tauri/src/memory/     mod.rs (SQLite), summarize.rs
src-tauri/src/actions/    mod.rs (tools, permissions, confirmations, log), clip.rs, input.rs, mousehook.rs, screen.rs
src-tauri/src/audio/      mod.rs (outputs, Mic-Link, stereo music), synth.rs (blips), tts.rs (Windows/Kokoro/OpenAI), kokoro.rs, rename.rs, commands.rs
src-tauri/src/music/      mod.rs (queue, player), ytdlp.rs, decode.rs (symphonia), analyze.rs (spectrum and sprites), lyrics.rs (LRCLIB, NetEase, subtitles)
src-tauri/src/            web.rs (DeepSeek Search/Fetch), costs.rs (cost log), i18n.rs (backend IT/EN texts)
src/                      index.html, style.css, app.js, dialog.js, face.js, state.js, chat.js, audio.js, sprites.js
src/                      settings.html, settings.css, settings.js, player.html, player.css, player.js, i18n.js (IT→EN translations)
persona/                  alter_ego.md (default character sheet)
tools/                    process_sprites.py, mock_openai.py, make_icon.py, build-windows.sh
tools/kokoro/             server.py (OpenAI-compatible Kokoro server), setup.ps1 (installation)
src/assets/sprites/       the 8 processed sprites
assets/sprites/raw/       original sprite sheet
docs/                     PERMESSI.md, MANCANTI.md, SOURCES.md, screenshots/ (screenshots/en/ = English UI)
```

The source code, comments and Italian UI strings are the originals; English texts live in `src/i18n.js` (frontend,
keyed by the Italian text) and in the `tr()` / `tr!` calls of the backend.

The sprites are included: `npm run build` directly produces the version with Alter Ego's face.

Releases are created from GitHub: Actions → *Release* → *Run workflow* (branch `main`), or by pushing to the `release` branch.
The version comes from `src-tauri/tauri.conf.json`.

## Known limitations

- Developed and tested on Linux, with the Windows build included; still to be verified on real Windows:
  - Windows voices (WinRT), Mic-Link with VB-CABLE and the device rename, Kokoro installation;
  - simulated keys for clips and mouse push-to-talk;
  - notifications, start with Windows, screenshots and brightness;
  - resource usage (goal: < 150 MB of RAM at rest, CPU ≈ 0%, startup < 2 s).
- Push-to-talk is required: there's no always-on listening with a wake word.
- The source sprite sheet has no blink or closed mouth: without those images, talking is a slight bounce.
- Mic-Link needs VB-CABLE: Windows doesn't allow creating a virtual microphone without a driver.
- Kokoro on CPU takes about half a second per sentence on a recent PC; the first sentence after startup waits for the model to load.
- The character sheet was written without access to the wiki pages: feel free to enrich it.
- Unsigned build: SmartScreen may show a warning.
- The pet reads open windows only on Windows (still to be verified on a real PC); on Linux he/she uses only the bottom of the screen.

## Credits and licenses

- **Code**: [MIT](LICENSE) license.
- **Alter Ego, Chihiro Fujisaki and Danganronpa** are © Spike Chunsoft. This is an unofficial fan project,
  not affiliated with or endorsed by Spike Chunsoft. The character sprites belong to their owners and are not covered by the MIT license.
- **Kokoro-82M** and hexgrad's `kokoro` library: Apache-2.0. Not bundled with the app: the Kokoro installation downloads them.
- **LFM3** by [Liquid AI](https://www.liquid.ai/blog/lfm3): LFM Open License v1.0. Not bundled: Ollama downloads it when you choose it.
- **[Ollama](https://github.com/ollama/ollama)**: MIT, not bundled (installed from ollama.com when you ask). **[Graphify](https://github.com/Graphify-Labs/graphify)** by Graphify Labs: Apache-2.0, installed with uv when you ask.
- SQL Server driver [tiberius](https://github.com/prisma/tiberius) (MIT/Apache-2.0).
- **VB-CABLE** by VB-Audio Software: donationware, not bundled; download it from the official website.
- **Font** [DotGothic16](https://github.com/fontworks-fonts/DotGothic16): SIL Open Font License 1.1 (`src/assets/fonts/OFL.txt`).
- Extra-content office backgrounds (`src/assets/extras/`): made by the project author with a Garry's Mod map.
- **Lambda Players** by StarFrost: optional, not included; Alter Ego Link only uses its public hooks and functions.
- **FlyWire connectome** (v783, [CC-BY 4.0](https://creativecommons.org/licenses/by/4.0/)): Dorkenwald et al., *Neuronal wiring diagram of an adult brain*, Nature 2024; Schlegel et al., *Whole-brain annotation and multi-connectome cell typing of Drosophila*, Nature 2024 ([data](https://zenodo.org/records/10676866), [annotations](https://github.com/flyconnectome/flywire_annotations)). Model parameters from Shiu et al., *A Drosophila computational brain model reveals sensorimotor processing*, Nature 2024. `src-tauri/resources/flybrain.bin` is a subset of the FlyWire data (active neurons only, synapse signs from the predicted neurotransmitters).
- **Alter Ego Link** (`src-tauri/gmod_addon/`): written for this project; some ideas (NPC body, mouth movement, event log) come from [AI-NPCs-GMod](https://github.com/SonnyTaylor/AI-NPCs-GMod) by SonnyTaylor (MIT).
- **ViaProxy bridge** (downloaded at runtime, not bundled): [ViaProxy](https://github.com/ViaVersion/ViaProxy) by RK_01 and the ViaVersion team, with [ViaVersion](https://github.com/ViaVersion/ViaVersion), [ViaBackwards](https://github.com/ViaVersion/ViaBackwards), [ViaRewind](https://github.com/ViaVersion/ViaRewind) and ViaLegacy (GPL-3.0). Portable Java: [Eclipse Temurin](https://adoptium.net) (GPL-2.0 with Classpath Exception).
- **Minecraft bot** (`src-tauri/mc_bot/`): written for this project on top of [Mineflayer](https://github.com/PrismarineJS/mineflayer) and its plugins mineflayer-pathfinder, mineflayer-pvp, mineflayer-collectblock, mineflayer-tool and prismarine-nbt for schematic files (PrismarineJS and contributors, MIT); some ideas (small composable skills, always-on modes) come from [Mindcraft](https://github.com/mindcraft-bots/mindcraft) (MIT). The "Alter Ego (Chihiro)" skin is original pixel art made with `tools/make_mc_skin.py`. Minecraft is a trademark of Mojang/Microsoft; this project is not affiliated with them.
- **VRChat integration** (`src-tauri/src/vrchat/`): written for this project (own small OSC encoder, no OSC crate). Ideas only — no code — from ProjectGabriel-Remastered (AGPL-3.0). VRChat is a trademark of VRChat Inc.; this project is not affiliated with it. OSC and chatbox addresses from VRChat's public OSC documentation; the web API endpoints from the community [vrchatapi](https://vrchatapi.github.io) documentation.
- **Chihiro pet** sprite sheet: by **Lucid On Bound** (`src/assets/pet/chihiro/`).
- **Alter Ego with headphones** sprite: original artwork by **Ayari** ([@xxayariii on Instagram](https://www.instagram.com/xxayariii/)),
  adapted for the app (background removed, green CRT tint). Thank you, Ayari! 💚
- Logo, circuit background, icon and sounds were created for this project.
- Built with [Tauri](https://tauri.app), [rusqlite](https://github.com/rusqlite/rusqlite), [cpal](https://github.com/RustAudio/cpal),
  [reqwest](https://github.com/seanmonstar/reqwest), [keyring](https://github.com/hwchen/keyring-rs), [symphonia](https://github.com/pdeljanov/Symphonia),
  [RustFFT](https://github.com/ejmahler/RustFFT) and other open-source libraries (see `src-tauri/Cargo.toml`).
