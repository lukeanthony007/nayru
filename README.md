## Description

A multi-provider text-to-speech service with audio queue management, provider abstraction, and dynamic server control via CLI or HTTP API.

## Skills / Tools / Stack

- TypeScript
- Azure Speech Services
- Google Cloud TTS
- Bun Runtime
- API Development

# Summary

Nayru is a text-to-speech service that abstracts multiple TTS providers behind a unified interface. Point it at Azure, Google, or a mock provider for testing—the application code stays the same.

The architecture follows a dynamic server app pattern. Run it as a persistent service with HTTP endpoints, or control it via CLI flags for scripted workflows. Audio queue management handles sequential playback without blocking.

Built as infrastructure for voice-enabled applications. Integrate with voice assistants, accessibility tools, or any system that needs text converted to speech with provider flexibility.

## Features

- Multi-provider TTS abstraction with Azure and Google backends
- Audio queue management with sequential playback
- Dynamic server app framework—CLI and HTTP control
- Wayland clipboard integration for reading selected text
- Zod-validated schema binding for type-safe configuration
- Auto-exposed class methods as HTTP endpoints
- Provider selection via environment variable
- Mock TTS provider for testing without API keys
- Graceful error handling with fallback behavior
- Bun runtime for fast startup and low overhead

### Roadmap

1. Add ElevenLabs provider for high-quality voice synthesis
2. Implement voice caching to reduce API calls
3. Build SSML support for pronunciation control
4. Create queue inspection and manipulation endpoints
5. Add streaming output for long-form text

### Instructions

1. Clone the repository and install dependencies with `bun install`
2. Set `TTS_PROVIDER` environment variable to `azure`, `google`, or `mock`
3. Configure provider-specific credentials in environment variables
4. Start the service with `bun run start`
5. Use CLI commands or HTTP API to queue text for speech

### License

MIT
