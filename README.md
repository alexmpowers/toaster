<p align="center">
  <img src="src/assets/toaster.png" alt="Toaster" width="200" />
</p>

<p align="center">
  <img src="src/assets/toaster_text.svg" alt="Toaster" width="200" />
</p>

<p align="center">
  <strong>Helping you sound crispy.</strong><br/>
  Edit video by editing text — entirely on your machine.
</p>

<p align="center">
  <a href="https://github.com/alexmpowers/toaster/releases"><img src="https://img.shields.io/github/v/release/alexmpowers/toaster?include_prereleases&label=latest&style=flat-square" alt="Latest release" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/github/license/alexmpowers/toaster?style=flat-square" alt="License" /></a>
  <a href="https://github.com/alexmpowers/toaster/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/alexmpowers/toaster/ci.yml?branch=main&style=flat-square&label=ci" alt="CI status" /></a>
</p>

<p align="center">
  <a href="#features">Features</a> ·
  <a href="#how-it-works">How It Works</a> ·
  <a href="#quick-start">Quick Start</a> ·
  <a href="#contributing">Contributing</a>
</p>

---

## Why Toaster?

Recording yourself is easy. Editing out every "um", false start, and awkward pause? That's the hard part.

Toaster is a **transcript-first** desktop editor for spoken audio and video. Instead of scrubbing a timeline, you read your words, select the ones you don't want, and delete them — just like editing a document. Toaster handles the audio splicing, waveform sync, and caption export behind the scenes.

Everything runs locally. No cloud APIs, no uploads, no subscriptions.

## Features

- **Edit media by editing text** — see your transcript, select words, delete/silence/restore in one click
- **Local transcription** — generate word-level transcripts with on-device models (Whisper ecosystem)
- **Filler & disfluency detection** — automatically highlight "um", "uh", "you know", and pauses
- **Non-destructive editing** — every action is reversible; your original file is never touched
- **Synchronized playback** — transcript, waveform, and video stay in lockstep as you edit
- **Export cleaned media** — render your final cut with captions (SRT/VTT) and script text
- **Save & resume** — project files preserve your edits for iterative sessions
- **Privacy-first** — no runtime network calls, no telemetry, fully offline

## How It Works

1. **Open** a video or audio file
2. **Transcribe** with a local model — Toaster generates a word-level transcript
3. **Read and edit** — select words you want to remove and hit Delete
4. **Preview** — play back your edit in real time with synced waveform and video
5. **Export** — render the cleaned media plus captions and script

The entire workflow stays on your machine. Your media never leaves your computer.

## Quick Start

### Install from release

Download the latest installer from the [Releases](https://github.com/alexmpowers/toaster/releases) page.

| Platform | Format               |
| -------- | -------------------- |
| Windows  | `.msi` / `.exe`      |
| macOS    | `.dmg`               |
| Linux    | `.AppImage` / `.deb` |

### Build from source

See [docs/build.md](docs/build.md) for full platform setup. The short version:

```bash
bun install --frozen-lockfile
cargo tauri dev
```

On Windows, run `.\scripts\setup-env.ps1` first to configure the MSVC + LLVM build environment.

## Tech Stack

| Layer         | Technology                                |
| ------------- | ----------------------------------------- |
| Desktop shell | [Tauri 2.x](https://tauri.app/)           |
| Backend       | Rust                                      |
| Frontend      | React · TypeScript · Tailwind CSS         |
| State         | Zustand                                   |
| Transcription | Local model inference (Whisper ecosystem) |
| Export        | FFmpeg 7                                  |

## Contributing

We welcome contributions! Please read [CONTRIBUTING.md](CONTRIBUTING.md) before opening a PR.

```bash
# Run the checks contributors are expected to pass
cd src-tauri && cargo test && cargo clippy
npm run lint
```

For translation contributions, see [CONTRIBUTING_TRANSLATIONS.md](CONTRIBUTING_TRANSLATIONS.md).

## Acknowledgments

Toaster is forked from [Handy](https://github.com/cjpais/Handy) by [CJ Pais](https://github.com/cjpais). Handy proved that a free, open-source, fully-offline speech tool could be simple, private, and community-driven. Toaster builds on that foundation with a transcript-first editing workflow.

We're grateful to the projects that make Toaster possible:

- [Tauri](https://tauri.app/) — the Rust-native app framework that keeps the bundle small and the runtime fast
- [Whisper](https://github.com/openai/whisper) by OpenAI — the speech recognition model at the heart of local transcription
- [whisper.cpp](https://github.com/ggerganov/whisper.cpp) & [ggml](https://github.com/ggerganov/ggml) — cross-platform inference and hardware acceleration
- [FFmpeg](https://ffmpeg.org/) — the Swiss Army knife of media processing

## License

MIT — see [LICENSE](LICENSE) for details.
