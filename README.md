<div align="center">

<img src="src-tauri/icons/icon.png" width="96" alt="Translate Bubble icon" />

# Translate Bubble

**Type in Hindi, Hinglish or broken English, click once, and send clean English.**

A small floating desktop widget, built with Rust and Tauri. It turns whatever you are typing in *any* app into clear, grammatically correct English and replaces the text for you.

![Rust](https://img.shields.io/badge/Rust-Tauri%20v2-orange) ![Size](https://img.shields.io/badge/installer-~1.3%20MB-green) ![RAM](https://img.shields.io/badge/RAM-~30%20MB-blue) ![License](https://img.shields.io/badge/license-MIT-lightgrey)

</div>

---

## The concept

Many of us think in Hindi or Hinglish but write to ChatGPT, Claude, clients, colleagues or WhatsApp groups in English. Switching to a translator tab and copying text back and forth breaks your flow.

**Translate Bubble removes that step.** A small round bubble floats above every window:

```
You type:   kal meeting h kya 5 baje? mujhe thoda late hoga
Click 🔵
You get:    Is there a meeting tomorrow at 5? I'll be a little late.
```

It works the same in a browser, desktop app, chat box, email or prompt box.

### Design principles
- **Faithful, not creative.** It keeps your meaning, tone and wording and only fixes grammar, spelling and word order. It never answers or "improves" your message. If you type *"mujhe python script likh ke do"*, you get *"Write me a Python script"*, not a script.
- **Works everywhere.** There are no browser extensions or per-app integrations. It uses the clipboard and simulated key presses, so it works in any text field.
- **Lightweight.** It's native Rust with the system WebView, with no Electron and no bundled Chromium. The installer is about 1.3 MB and the app uses about 30 MB of RAM.
- **Free and private by default.** It can run fully offline with a local open-source model (Ollama). Cloud engines are optional.

---

## Features

| | |
|---|---|
| 🫧 **Floating bubble** | Always on top. Drag it anywhere. |
| ⚡ **One-click convert** | Click the bubble or press `Ctrl+Shift+Space`, and the text in the focused box is replaced with English. |
| ✂️ **Selection aware** | Select part of your text to convert only that part. With nothing selected, the whole box is converted. |
| 📋 **Copy mode** | Instead of replacing, it can just copy the result and show it in the panel. |
| 📝 **Manual panel** | Right-click the bubble to type or paste text, convert it, and copy the result. |
| 🔌 **Pluggable engines** | Ollama (local), any OpenAI-compatible API, or Google's free fallback. |
| 🖥️ **Tray icon** | Show or hide the bubble, open settings, quit. |

---

## How it works

```
┌──────────────┐  click / hotkey   ┌─────────────────────────────┐
│ Bubble (UI)  │ ────────────────▶ │ Rust core                   │
└──────────────┘                   │ 1. Give focus back to your  │
                                   │    last app (Win32)         │
                                   │ 2. Ctrl+C (or Ctrl+A,       │
                                   │    Ctrl+C if no selection)  │
                                   │ 3. Read the clipboard       │
                                   │ 4. Translate ───────────────┼──▶ Ollama / OpenAI-compatible / Google
                                   │ 5. Write result to clipboard│
                                   │ 6. Ctrl+V over the selection│
                                   └─────────────────────────────┘
```

- A background thread remembers the last window that wasn't the bubble, so clicking the bubble can hand focus straight back to it.
- Clipboard sequence numbers tell whether a copy actually happened, so there's no guessing.
- The app waits for hotkey modifiers to be released, so `Ctrl+C` doesn't become `Ctrl+Shift+C`.
- LLM engines get a strict "converter, not assistant" prompt with a few examples. Optional settings a model rejects (for example `temperature` on reasoning models) are removed and the request is retried automatically.

---

## Translation engines

| Engine | Cost | Privacy | Grammar fix | Setup |
|---|---|---|---|---|
| **Ollama** (local) | Free | 100% offline | ✅ | Install [Ollama](https://ollama.com), then run `ollama pull qwen2.5:3b` |
| **OpenAI-compatible** | Free tiers or paid | Sent to the provider | ✅ | Base URL, API key, model |
| **Google** (fallback) | Free | Sent to Google | ❌ translate only | None |

**Auto** (the default) uses Ollama and falls back to Google if Ollama isn't running.

Example OpenAI-compatible setups:

| Provider | Base URL | Model |
|---|---|---|
| Groq (free tier) | `https://api.groq.com/openai/v1` | `llama-3.1-8b-instant` |
| OpenRouter | `https://openrouter.ai/api/v1` | any `:free` model |
| OpenAI | `https://api.openai.com/v1` | `gpt-5-nano`, `gpt-4o-mini` |
| LM Studio (local) | `http://localhost:1234/v1` | your loaded model |

For better local quality, try `qwen2.5:7b` or `llama3.1:8b`.

---

## Install (users)

1. Download `Translate Bubble_x.y.z_x64-setup.exe` from Releases, or build it yourself (see below).
2. Run it. It installs per user, so no admin rights are needed, and it adds Desktop and Start Menu shortcuts.
3. Launch **Translate Bubble**. The bubble appears on the right edge of your screen.
4. Right-click the bubble, open **Settings**, choose an engine, and click **Save**.

Settings are stored in `%APPDATA%\tech.openforge.translatebubble\settings.json`. API keys are stored there in plain text, on your machine only.

### Usage

| Action | Result |
|---|---|
| Click inside any text box, then **click the bubble** | The text is replaced with English |
| `Ctrl+Shift+Space` | Same as clicking the bubble. You can change it in Settings. |
| **Drag** the bubble | Moves it |
| **Right-click** the bubble | Opens the panel (manual convert and settings) |
| Tray icon | Show or hide the bubble, open the panel, quit |

The bubble shows a spinner while working, turns green with ✓ when done, and turns red with ! on error. Errors also open the panel with a message.

---

## Run from source (developers)

### Prerequisites (Windows)
- [Rust](https://rustup.rs) (stable)
- [Node.js](https://nodejs.org) 18 or later (only for the Tauri CLI)
- Microsoft C++ Build Tools ("Desktop development with C++")
- WebView2 Runtime (preinstalled on Windows 10 and 11)

### Clone and run

```bash
git clone https://github.com/kasifraza/desktop-translator.git
cd desktop-translator
npm install
npm run dev
```

`npm run dev` starts the app in debug mode.

### Build the installer

```bash
npm run build
```

The installer is written to `src-tauri/target/release/bundle/nsis/`.

### Tests

```bash
cd src-tauri
cargo test
cargo test -- --include-ignored
```

`cargo test` runs the offline tests. `cargo test -- --include-ignored` adds the network tests; one of them uses your saved settings.

### Project structure

```
├── ui/
│   ├── index.html        # floating bubble (click / drag / right-click)
│   └── panel.html        # manual translate + settings panel
├── src-tauri/
│   ├── src/
│   │   ├── main.rs       # windows, tray, hotkey, commands, convert flow
│   │   ├── input.rs      # Win32: focus tracking, SendInput, clipboard
│   │   └── translate.rs  # prompt + Ollama / OpenAI-compatible / Google
│   ├── tauri.conf.json   # window + bundle config
│   └── capabilities/     # Tauri permissions
└── scripts/make-icons.mjs  # generates the app icon (no dependencies)
```

### Tech stack
- **Tauri v2** with Rust for the backend and the system WebView2 for the UI
- **Vanilla HTML, CSS and JS** with no framework and no bundler
- `windows-sys` for focus tracking and key presses, `arboard` for the clipboard, `reqwest` for HTTP
- Tauri plugins: `global-shortcut` and `single-instance`
- Release profile: `opt-level="s"`, LTO, stripped binary

---

## Limitations and roadmap
- Windows only for now. The key-press and focus code is Win32; the macOS and Linux versions need their own input code.
- Apps that copy the whole line when nothing is selected (for example VS Code) can behave unexpectedly. Select the text first in those apps.
- Ideas: remember the bubble's position, option to start with Windows, target languages other than English, a "rephrase politely" mode.

## License

MIT. See [LICENSE](LICENSE).
