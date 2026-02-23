## Description

Standalone voice server that turns text into speech. Any application sends text over HTTP or CLI — nayru cleans it, splits it into natural sentences, synthesizes audio through Kokoro TTS, and plays it back natively with gapless sequencing. Built as infrastructure for voice-enabled applications where the caller shouldn't have to think about audio pipelines.

## Stack / Tools

- Rust
- [Kokoros](https://github.com/lucasjinreal/Kokoros) (Rust-native Kokoro TTS, ONNX, OpenAI-compatible API)
- rodio (audio playback)
- HTTP API / CLI
- Native audio (ALSA/PulseAudio/PipeWire, CoreAudio)

## How it works

You send text, nayru handles the rest. Under the hood:

1. **Clean & split** — Markdown is stripped (code blocks, bold, headings, links, etc.) and the text is broken into individual sentences.
2. **Synthesize** — Each sentence is sent to a local Kokoro TTS server. Two workers run in parallel: while one streams the current sentence's audio to speakers, the other is already fetching the next sentence. This means the first audio plays in under a second, and subsequent sentences are ready by the time the previous one finishes.
3. **Play** — Raw PCM audio streams directly into a native audio sink (rodio) for gapless playback. No files, no buffering the whole response.

Stopping is instant — a single atomic counter invalidates all in-flight work.

## Features

- Text-to-speech pipeline with markdown stripping and sentence splitting
- Kokoro TTS backend (local, OpenAI-compatible `/v1/audio/speech`)
- PCM streaming with pipelined prefetch (~0.3-0.9s time-to-first-audio)
- Audio queue with gapless playback via rodio
- HTTP server (default port 2003) with permissive CORS
- CLI: `speak`, `stop`, `skip`, `pause`, `resume`, `status`
- Embeddable as a Rust library
- Configurable voice, speed, and Kokoro URL

### Instructions

1. Install and run [Kokoros](https://github.com/lucasjinreal/Kokoros) on port 3001:
   ```bash
   git clone https://github.com/lucasjinreal/Kokoros.git
   cd Kokoros
   bash download_all.sh
   cargo build --release
   ./target/release/koko --instances 1 openai --port 3001
   ```
2. Ensure an audio output device is available (ALSA/PulseAudio/PipeWire on Linux, CoreAudio on macOS)
3. Build with `cargo build --release` (binary at `target/release/nayru`)
4. Start the server with `nayru serve` or use the CLI / HTTP API

### License

MIT

## Usage

### Server mode

```bash
# Start the voice server (default: 127.0.0.1:2003)
nayru serve

# With options
nayru serve --port 2003 --voice af_heart --kokoro-url http://localhost:3001 --speed 1.0
```

### Client commands

```bash
# Send text to speak
nayru speak "Hello, this is nayru."

# Control playback
nayru stop      # Stop all speech, clear queue
nayru skip      # Skip current clip
nayru pause     # Pause playback
nayru resume    # Resume playback
nayru status    # Get current state
```

### HTTP API

All endpoints served on port 2003 with permissive CORS.

| Endpoint  | Method | Body                                   | Response                              |
|-----------|--------|----------------------------------------|---------------------------------------|
| `/speak`  | POST   | `{"text": "...", "voice": "af_heart"}` | `{"ok": true, "queued_chunks": 3}`    |
| `/stop`   | POST   | —                                      | `{"ok": true}`                        |
| `/skip`   | POST   | —                                      | `{"ok": true}`                        |
| `/pause`  | POST   | —                                      | `{"ok": true}`                        |
| `/resume` | POST   | —                                      | `{"ok": true}`                        |
| `/status` | GET    | —                                      | `{"state": "playing", "queue_length": 2, "voice": "af_heart"}` |

```bash
curl -X POST localhost:2003/speak -H 'Content-Type: application/json' -d '{"text":"Hello from curl"}'
curl localhost:2003/status
```

### As a library

```rust
use nayru::tts::{TtsEngine, TtsConfig};

let engine = TtsEngine::new(TtsConfig::default());
engine.speak("Hello world.");
engine.status();  // TtsStatus { state: Playing, queue_length: 0, voice: "af_heart" }
engine.stop();
```

### Building

```bash
cargo build --release
```

The binary is at `target/release/nayru`.
