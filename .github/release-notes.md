## Alter Ego 1.8.2

- Lighter at rest: the speakers pause when there's nothing to play, the music player and Venue no longer redraw 60 times a second, the Garry's Mod sound index is freed when the game closes.
- Push-to-talk from the keyboard no longer hooks the mouse (no extra mouse latency in games).
- More stable: a crash in one part no longer breaks the others (locks recover), the action log is capped at 5 MB.
- All tests pass on Windows too.

`AlterEgo_1.8.2_setup.exe` / `AlterEgo_1.8.2_portable.zip` Windows · `AlterEgo_1.8.2_amd64.AppImage` / `AlterEgo_1.8.2_amd64.deb` Linux · `SHA256SUMS.txt`

## Alter Ego 1.8.1

- Fix: the model catalog window opened blank on Windows.

`AlterEgo_1.8.1_setup.exe` / `AlterEgo_1.8.1_portable.zip` Windows · `AlterEgo_1.8.1_amd64.AppImage` / `AlterEgo_1.8.1_amd64.deb` Linux · `SHA256SUMS.txt`

## Alter Ego 1.8.0

- Local models: Gemma 4 and Gemma 3 among the recommended ones, catalog in its own window, tools work again (16K/32K context), Hugging Face quantization links fixed, long names no longer overflow.
- Hermes Agent: one-click install, long tasks delegated (with confirmation), optional "Hermes as the brain".
- VoiceStudio as a voice, with setup and autostart. Rizzo Flow from an existing folder.
- SQL Server LocalDB, camera-shutter animation on screenshots, push-to-talk that works inside games and shows her right away, waits for slow pages.
- Memory: Compact button and automatic cleanup. Alter Ego more in character.
- Garry's Mod (Shinri Trial): she reacts to the game's sounds (off by default).

`AlterEgo_1.8.0_setup.exe` / `AlterEgo_1.8.0_portable.zip` Windows · `AlterEgo_1.8.0_amd64.AppImage` / `AlterEgo_1.8.0_amd64.deb` Linux · `SHA256SUMS.txt`

## Alter Ego 1.7.0

- Local models: LFM3 by Liquid AI in one click, Ollama installed for you, model browser with filters (abliterated, uncensored, tools, vision…), download from Hugging Face or a .gguf address.
- AMD Radeon RX 6600 and newer: Vulkan, choice of the dedicated card on PCs with Intel integrated graphics, smaller context cache.
- Fewer tokens in complex requests: tools in groups loaded on demand, old tool results compacted, automatic thinking, files read in pieces.
- She knows which apps are open (names only, cache-friendly). Project maps with Graphify.
- SQL Server windows restyled like Alter Ego.

`AlterEgo_1.7.0_setup.exe` / `AlterEgo_1.7.0_portable.zip` Windows · `AlterEgo_1.7.0_amd64.AppImage` / `AlterEgo_1.7.0_amd64.deb` Linux · `SHA256SUMS.txt`

## Alter Ego 1.6.0

- More precise clicks: screenshots with coordinate rulers and the cursor marked, zoom, check after each click, exact controls on Windows.
- SQL Server on your PC: connection form (the password stays local), queries, results window.
- Complex requests: up to 12 tool steps (adjustable); Esc stops an answer; no more stalls on hung tools.
- Terminal: bash on Linux, keeps the working folder, adjustable timeout. Optional access to the whole disk.
- Silent start in the tray, "always on top" switch, full log in Settings → Log.
- Ollama: context window, keep-alive, GPU layers; max tokens, top P, timeout. Linux texts no longer mention Windows.

`AlterEgo_1.6.0_setup.exe` / `AlterEgo_1.6.0_portable.zip` Windows · `AlterEgo_1.6.0_amd64.AppImage` / `AlterEgo_1.6.0_amd64.deb` Linux · `SHA256SUMS.txt`

## Alter Ego 1.5.0

- First Linux release: AppImage and .deb (x86-64).
- Rizzo Flow: "Install" button in Settings → AI, and the server starts with Alter Ego.

`AlterEgo_1.5.0_setup.exe` / `AlterEgo_1.5.0_portable.zip` Windows · `AlterEgo_1.5.0_amd64.AppImage` / `AlterEgo_1.5.0_amd64.deb` Linux · `SHA256SUMS.txt`

## Alter Ego 1.4.1

- Alter Net inside Alter Ego: "open Alter Net", tray or Settings → General. Borderless fullscreen, F11 windowed, Esc twice closes.
- Offline single player when the server is down (the copy is downloaded from the server the first time you play online).

`AlterEgo_1.4.1_setup.exe` installer · `AlterEgo_1.4.1_portable.zip` portable · `SHA256SUMS.txt`

## Alter Ego 1.4.0

- Fix: tool calls written as text (DeepSeek "DSML" etc.) are executed, not shown; `<think>` hidden.
- Thinking: hide or turn off.
- Token saving: tools only when needed, local router (keywords or Rizzo Flow), cheap model, compact history.
- Web search: DeepSeek, Google (Gemini), Tavily, Brave or DuckDuckGo, with fallback; free local page reader.
- Speech recognition: Google Gemini, and fallback Groq ⇄ Gemini.
- Spotify (Premium) for "play…".
- Music: YouTube titles as they are; player and Venue layout fixes.

`AlterEgo_1.4.0_setup.exe` installer · `AlterEgo_1.4.0_portable.zip` portable · `SHA256SUMS.txt`

## Alter Ego 1.3.3

- Headphones sprite much bigger in the main window (still whole, neck included).

`AlterEgo_1.3.3_setup.exe` installer · `AlterEgo_1.3.3_portable.zip` portable · `SHA256SUMS.txt`

## Alter Ego 1.3.2

- Headphones sprite: whole neck and collar visible.
- Player ⇄ Music Venue: animated resize with a glitch transition; detach/dock animated too.
- Player and Venue never cover Alter Ego or each other (new placement).

`AlterEgo_1.3.2_setup.exe` installer · `AlterEgo_1.3.2_portable.zip` portable · `SHA256SUMS.txt`

## Alter Ego 1.3.1

- Player expands into the Music Venue (same window) and back; ⧉ detaches it into its own window.
- Fix: crash/freeze when closing and reopening the Music Venue.
- Fix: headphones sprite no longer cut off.

`AlterEgo_1.3.1_setup.exe` installer · `AlterEgo_1.3.1_portable.zip` portable · `SHA256SUMS.txt`

## Alter Ego 1.3.0

- **Music Venue**: recent tracks and favorites, search, replay.
- Headphones sprite while music plays.
- Player: resizable, opens next to Alter Ego, glitch open/close, smoother volume (lower max, 2% steps).
- App commands by voice/text ("open music venue", "close the player"…).
- Mouse and keyboard control (permission, off by default).
- Mic-Link noise suppression (RNNoise).
- **Unfiltered** mode.
- Website/GitHub links and in-app update.

`AlterEgo_1.3.0_setup.exe` installer · `AlterEgo_1.3.0_portable.zip` portable · `SHA256SUMS.txt`
