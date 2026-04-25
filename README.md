# DawnChat API

Backend services and core Rust crates that power the DawnChat platform.

[![Stars](https://img.shields.io/github/stars/Chillboio-Studios/dawnchat-api?style=flat-square&logoColor=white)](https://github.com/Chillboio-Studios/dawnchat-api/stargazers)
[![Forks](https://img.shields.io/github/forks/Chillboio-Studios/dawnchat-api?style=flat-square&logoColor=white)](https://github.com/Chillboio-Studios/dawnchat-api/network/members)
[![Pull Requests](https://img.shields.io/github/issues-pr/Chillboio-Studios/dawnchat-api?style=flat-square&logoColor=white)](https://github.com/Chillboio-Studios/dawnchat-api/pulls)
[![Issues](https://img.shields.io/github/issues/Chillboio-Studios/dawnchat-api?style=flat-square&logoColor=white)](https://github.com/Chillboio-Studios/dawnchat-api/issues)
[![Contributors](https://img.shields.io/github/contributors/Chillboio-Studios/dawnchat-api?style=flat-square&logoColor=white)](https://github.com/Chillboio-Studios/dawnchat-api/graphs/contributors)
[![License](https://img.shields.io/github/license/Chillboio-Studios/dawnchat-api?style=flat-square&logoColor=white)](https://github.com/Chillboio-Studios/dawnchat-api/blob/main/LICENSE)

## What This Repository Contains

This workspace includes API servers, websocket/event services, supporting daemons, and reusable core crates.

### Core crates

- `crates/core/config`: shared configuration models and loading.
- `crates/core/database`: database abstractions, drivers, and tasks.
- `crates/core/files`: file handling and storage helpers.
- `crates/core/models`: API and internal data models.
- `crates/core/parser`: parser utilities.
- `crates/core/permissions`: permission evaluation logic.
- `crates/core/presence`: presence state handling.
- `crates/core/ratelimits`: rate limit types and logic.
- `crates/core/result`: common result/error types.
- `crates/core/coalesced`: coalesced service crate.

### Runtime services

- `crates/delta`: REST API server.
- `crates/bonfire`: websocket events server.
- `crates/services/autumn`: file server.
- `crates/services/january`: proxy server.
- `crates/services/gifbox`: GIF/Tenor proxy service.
- `crates/daemons/crond`: scheduled background tasks.
- `crates/daemons/pushd`: push notification daemon.
- `crates/daemons/voice-ingress`: voice ingress daemon.

## Requirements

- Rust toolchain compatible with the repository toolchain config.
- Docker and Docker Compose.
- Git.
- `mise` (recommended for local developer workflows).
- `mold` (optional, for faster local builds).

For Nix users, `default.nix` is available.

## Quick Start (Development)

```bash
git clone https://github.com/Chillboio-Studios/dawnchat-api
cd dawnchat-api

mise install
mise build
cp livekit.example.yml livekit.yml
mise start
```

When signing up locally, open `http://localhost:14080` to view confirmation and password reset mail.

To stop services, interrupt `mise start` and run:

```bash
mise docker:stop
```

## Default Local Ports

| Service                                | Port         |
| -------------------------------------- | ------------ |
| MongoDB                                | 27017        |
| Redis                                  | 6379         |
| MinIO                                  | 14009        |
| Maildev                                | 14025, 14080 |
| RabbitMQ                               | 5672, 15672  |
| API (`crates/delta`)                   | 14702        |
| Events (`crates/bonfire`)              | 14703        |
| Files (`crates/services/autumn`)       | 14704        |
| Proxy (`crates/services/january`)      | 14705        |
| GIF service (`crates/services/gifbox`) | 14706        |

## Configuration

- Base config file: `Revolt.toml`.
- Local overrides: create `Revolt.overrides.toml`.
- Test overrides: `Revolt.test-overrides.toml` and optional local override companion.

Example Sentry configuration override:

```toml
[sentry]
api = "https://abc@your.sentry/1"
events = "https://abc@your.sentry/1"
files = "https://abc@your.sentry/1"
proxy = "https://abc@your.sentry/1"
```

If you use custom service ports, update both Docker compose mappings and relevant override files.

## OAuth2 Login (Optional)

Delta now exposes OAuth2 login routes at:

- `GET /auth/oauth2/enabled`
- `GET /auth/oauth2/authorize?redirect_uri=<client-callback>`
- `GET /auth/oauth2/callback`
- `POST /auth/oauth2/exchange`

To enable OAuth2, set these environment variables for the API process:

- `DAWNCHAT_OAUTH2_CLIENT_ID`
- `DAWNCHAT_OAUTH2_AUTHORIZE_URL`
- `DAWNCHAT_OAUTH2_TOKEN_URL`
- `DAWNCHAT_OAUTH2_USERINFO_URL`

Optional:

- `DAWNCHAT_OAUTH2_CLIENT_SECRET`
- `DAWNCHAT_OAUTH2_SCOPE` (default: `openid profile email`)
- `DAWNCHAT_OAUTH2_PROVIDER_NAME` (default: `OAuth2`)
- `DAWNCHAT_OAUTH2_CALLBACK_URL` (default: `<hosts.api>/auth/oauth2/callback`)
- `DAWNCHAT_OAUTH2_EMAIL_FIELD` (default: `email`)
- `DAWNCHAT_OAUTH2_STATE_SECRET` (falls back to `api.security.authifier_shield_key`)

## Building

Build all binaries in release mode:

```bash
cargo build --release --bins
```

Optional mold builder override in `.env`:

```bash
BUILDER = "mold --run cargo"
```

## Testing

Start dependencies:

```bash
docker compose up -d
```

Run test suites:

```bash
TEST_DB=REFERENCE cargo nextest run
TEST_DB=MONGODB cargo nextest run
```

## Docker Workflows

This repository contains CI workflows for:

- Build-only image validation on pull requests.
- Multi-image publish on non-PR events.
- Shared build cache for faster image rebuilds.

## Releases

Version bump helpers:

```bash
just patch
just minor
just major
```

Publish crates:

```bash
just publish
```

Tag a new binary release:

```bash
just release
```

## Contributing

Contributions are welcome. Open an issue or pull request with:

- a clear problem statement,
- implementation details,
- and testing notes.

Before opening a PR, run formatting, lints, and tests relevant to your changes.

## License

This project is generally licensed under the GNU Affero General Public License v3.0.

See [LICENSE](LICENSE) for details. Some crates may include their own license files.

Based on StoatChat
