## Description

Standalone voice server that turns text into speech. Any application sends text over HTTP or CLI — nayru cleans it, splits it into natural sentences, synthesizes audio through Kokoro TTS, and plays it back natively with gapless sequencing. Built as infrastructure for voice-enabled applications where the caller shouldn't have to think about audio pipelines.

## Stack / Tools

- Rust
- Kokoro TTS (OpenAI-compatible API)
- rodio (audio playback)
- HTTP API / CLI
- Native audio (ALSA/PulseAudio/PipeWire, CoreAudio)

# Summary

Nayru is a voice server built as a three-task actor pipeline connected by bounded channels:

```
text → clean/split/merge → POST Kokoro /v1/audio/speech → rodio Sink playback
         (Task 1)                  (Task 2)                  (Task 3, OS thread)
```

- **Text processing** — strips markdown (code blocks, tables, bold, headings, links, lists), splits at sentence boundaries, merges small chunks for fewer API calls
- **Kokoro fetcher** — concurrent WAV fetches from local Kokoro TTS (OpenAI-compatible API)
- **Playback** — rodio on a dedicated OS thread for gapless audio scheduling
- **Epoch-based cancellation** — `stop()` bumps an atomic counter, all in-flight work for the previous epoch is silently discarded

## Features

- Text-to-speech pipeline with markdown stripping and sentence splitting
- Kokoro TTS backend (local, OpenAI-compatible `/v1/audio/speech`)
- Audio queue with gapless playback via rodio
- HTTP server (default port 2003) with permissive CORS
- CLI: `speak`, `stop`, `skip`, `pause`, `resume`, `status`
- Embeddable as a Rust library
- Configurable voice, speed, and Kokoro URL

### Instructions

1. Run [Kokoro TTS](https://github.com/remsky/Kokoro-FastAPI) on port 8880 (OpenAI-compatible `/v1/audio/speech` endpoint)
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
nayru serve --port 2003 --voice af_jadzia --kokoro-url http://localhost:8880 --speed 1.0
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

| Endpoint  | Method | Body                                     | Response                              |
|-----------|--------|------------------------------------------|---------------------------------------|
| `/speak`  | POST   | `{"text": "...", "voice": "af_jadzia"}`  | `{"ok": true, "queued_chunks": 3}`    |
| `/stop`   | POST   | —                                        | `{"ok": true}`                        |
| `/skip`   | POST   | —                                        | `{"ok": true}`                        |
| `/pause`  | POST   | —                                        | `{"ok": true}`                        |
| `/resume` | POST   | —                                        | `{"ok": true}`                        |
| `/status` | GET    | —                                        | `{"state": "playing", "queue_length": 2, "voice": "af_jadzia"}` |

```bash
curl -X POST localhost:2003/speak -H 'Content-Type: application/json' -d '{"text":"Hello from curl"}'
curl localhost:2003/status
```

### As a library

```rust
use nayru::tts::{TtsEngine, TtsConfig};

let engine = TtsEngine::new(TtsConfig::default());
engine.speak("Hello world.");
engine.status();  // TtsStatus { state: Playing, queue_length: 0, voice: "af_jadzia" }
engine.stop();
```

### Building

```bash
cargo build --release
```

The binary is at `target/release/nayru`.
