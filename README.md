# nayru

Voice server — send text in, get speech out. Handles the full pipeline: text cleaning, sentence splitting, Kokoro TTS synthesis, and native audio playback via rodio. Run it as an HTTP server and control it from any app, or use the CLI directly.

## Architecture

Three-task actor pipeline connected by bounded channels:

```
text → clean/split/merge → POST Kokoro /v1/audio/speech → rodio Sink playback
         (Task 1)                  (Task 2)                  (Task 3, OS thread)
```

- **Text processing** — strips markdown (code blocks, tables, bold, headings, links, lists), splits at sentence boundaries, merges small chunks for fewer API calls
- **Kokoro fetcher** — concurrent WAV fetches from local Kokoro TTS (OpenAI-compatible API)
- **Playback** — rodio on a dedicated OS thread for gapless audio scheduling
- **Epoch-based cancellation** — `stop()` bumps an atomic counter, all in-flight work for the previous epoch is silently discarded

## Requirements

- [Kokoro TTS](https://github.com/remsky/Kokoro-FastAPI) running on port 8880 (OpenAI-compatible `/v1/audio/speech` endpoint)
- Audio output device (ALSA/PulseAudio/PipeWire on Linux, CoreAudio on macOS)

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

## Building

```bash
cargo build --release
```

The binary is at `target/release/nayru`.

## License

MIT
