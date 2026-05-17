# Pebble Web — Design Spec

## Overview

Pebble Web is an independent web-based email client derived from the Pebble desktop app. It provides browser-based access to email with Docker deployment support, targeting single-user self-hosted scenarios.

**Origin:** GitHub Issue [#33](https://github.com/QingJ01/Pebble/issues/33) — "能支持web方式吗，通过docker部署"

## Decisions

| Decision | Choice |
|----------|--------|
| Repository | Independent repo (separate from desktop Pebble) |
| Backend | Rust (Axum) |
| Frontend | React (adapted from existing Tauri frontend) |
| Database | SQLite (single-user self-hosted) |
| Feature scope | Near-desktop: inbox, folders, send, attachments, labels/stars, search, translate |
| Authentication | Simple password + JWT |
| Account config | Web UI (OAuth callbacks point to web server) |
| Code reuse | Copy existing crates into new repo |

## Architecture

```
┌─────────────────────────────────────────────┐
│              Docker Container                │
│                                             │
│  ┌───────────────────────────────────────┐  │
│  │         Axum HTTP Server              │  │
│  │  ┌─────────┐  ┌──────────────────┐   │  │
│  │  │ Static  │  │   REST API       │   │  │
│  │  │ Files   │  │   /api/v1/...    │   │  │
│  │  │ (React) │  │                  │   │  │
│  │  └─────────┘  └──────────────────┘   │  │
│  │         ┌──────────────────┐          │  │
│  │         │  WebSocket       │          │  │
│  │         │  (realtime sync) │          │  │
│  │         └──────────────────┘          │  │
│  └───────────────────────────────────────┘  │
│                     │                        │
│  ┌──────────┬───────┼───────┬────────────┐  │
│  │pebble-   │pebble-│pebble-│pebble-     │  │
│  │store     │mail   │search │translate   │  │
│  └──────────┴───────┴───────┴────────────┘  │
│                     │                        │
│         ┌───────────┴───────────┐           │
│         │  SQLite + Tantivy     │           │
│         │  /data/               │           │
│         └───────────────────────┘           │
└─────────────────────────────────────────────┘
```

Single process. Axum serves both the React static bundle and the REST/WebSocket API. Data persists in a Docker volume at `/data/`.

## API Design

### Authentication

```
POST /api/v1/auth/login       → { token: string }
POST /api/v1/auth/logout
```

- Login with password, returns JWT
- Password stored as bcrypt/argon2 hash in config.json (set on first launch or via env)
- JWT in `Authorization: Bearer <token>` header for all subsequent requests
- JWT validity configurable (default 7 days)
- WebSocket auth via query param `?token=<jwt>`

### Accounts

```
GET    /api/v1/accounts
POST   /api/v1/accounts
DELETE /api/v1/accounts/:id
POST   /api/v1/accounts/:id/oauth
```

### Messages & Threads

```
GET    /api/v1/folders
GET    /api/v1/messages?folder=&page=&limit=
GET    /api/v1/messages/:id
PATCH  /api/v1/messages/:id/flags
POST   /api/v1/messages/:id/move
DELETE /api/v1/messages/:id
GET    /api/v1/threads/:id
```

### Compose

```
POST   /api/v1/compose/send
POST   /api/v1/compose/draft
GET    /api/v1/drafts
POST   /api/v1/compose/attachments
GET    /api/v1/attachments/:id
```

### Search & Translate

```
GET    /api/v1/search?q=...
POST   /api/v1/translate
```

### Sync & Realtime

```
POST   /api/v1/sync/trigger
WS     /api/v1/ws
```

WebSocket pushes: new mail notifications, sync progress, folder count updates.

## Frontend Adaptation

### Communication Layer Replacement

All `invoke()` calls replaced with HTTP requests via a centralized `api-client.ts`:

```typescript
// api-client.ts
const api = axios.create({ baseURL: '/api/v1' });
api.interceptors.request.use((config) => {
  config.headers.Authorization = `Bearer ${getToken()}`;
  return config;
});
```

Tauri event listeners (`listen()`) replaced with WebSocket message subscriptions.

### Modules to Remove

- `showMainWindow.ts` — Tauri window API
- `useCloseToBackground.ts` — Tauri window behavior
- `useTrayI18n.ts` — system tray
- `startupTiming.ts` — Tauri-specific timing

### Modules to Adapt

- `useNotificationOpenNavigation.ts` → browser Notification API
- `useMailtoOpen.ts` → standard web mailto handling
- All `hooks/queries/` — replace `invoke` with `api-client` calls
- All `hooks/mutations/` — replace `invoke` with `api-client` calls

### Modules Unchanged

- All UI components (InboxView, ThreadView, ComposeToolbar, etc.)
- Zustand stores (mail, compose, kanban, etc.)
- Style system
- React Query hook structure (only transport layer changes)

## Project Structure

```
pebble-web/
├── Cargo.toml                 # workspace root
├── Dockerfile
├── docker-compose.yml
├── .env.example
│
├── crates/
│   ├── pebble-core/           # copied, remove Tauri deps
│   ├── pebble-store/          # copied as-is
│   ├── pebble-mail/           # copied as-is
│   ├── pebble-search/         # copied as-is
│   ├── pebble-translate/      # copied as-is
│   ├── pebble-crypto/         # copied, keyring → file/env-based key storage
│   └── pebble-oauth/          # copied, callback URI → web URL
│
├── src/                       # Axum server
│   ├── main.rs
│   ├── config.rs              # env + config.json loading
│   ├── auth.rs                # password verification + JWT
│   ├── state.rs               # AppState
│   ├── ws.rs                  # WebSocket handler
│   ├── sync.rs                # background IMAP sync task
│   └── routes/
│       ├── mod.rs
│       ├── accounts.rs
│       ├── messages.rs
│       ├── folders.rs
│       ├── compose.rs
│       ├── attachments.rs
│       ├── search.rs
│       └── translate.rs
│
└── frontend/                  # React app (adapted from Pebble/src)
    ├── package.json
    ├── vite.config.ts
    └── src/
        ├── api-client.ts      # new: HTTP/WS communication layer
        ├── App.tsx
        ├── components/
        ├── features/
        ├── hooks/
        ├── stores/
        └── lib/
```

### Crate Adaptation Notes

| Crate | Changes |
|-------|---------|
| pebble-core | Remove optional `tauri` feature if any |
| pebble-store | None expected |
| pebble-mail | None expected |
| pebble-search | None expected |
| pebble-translate | None expected |
| pebble-crypto | Remove `keyring` dep; read master key from env `PEBBLE_ENCRYPTION_KEY` or file |
| pebble-oauth | Change redirect URI to `http://<host>:8080/api/v1/accounts/:id/oauth/callback` |

## Docker Deployment

### Dockerfile (multi-stage)

```dockerfile
# Stage 1: Build React frontend
FROM node:20-alpine AS frontend
WORKDIR /app
COPY frontend/ .
RUN npm ci && npm run build

# Stage 2: Build Rust backend
FROM rust:1.80-alpine AS backend
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY src/ src/
RUN apk add musl-dev && cargo build --release

# Stage 3: Final image
FROM alpine:3.20
RUN apk add --no-cache ca-certificates
COPY --from=backend /app/target/release/pebble-web /usr/local/bin/
COPY --from=frontend /app/dist /usr/local/share/pebble-web/static
EXPOSE 8080
VOLUME /data
CMD ["pebble-web"]
```

### docker-compose.yml

```yaml
services:
  pebble-web:
    image: pebble-web:latest
    ports:
      - "8080:8080"
    volumes:
      - pebble-data:/data
    environment:
      - PEBBLE_PASSWORD=your-password
      - PEBBLE_JWT_SECRET=random-secret
      - PEBBLE_DATA_DIR=/data
      - PEBBLE_PORT=8080
      - PEBBLE_SYNC_INTERVAL=300
    restart: unless-stopped

volumes:
  pebble-data:
```

### Data Directory

```
/data/
├── pebble.db          # SQLite database
├── index/             # Tantivy full-text index
├── attachments/       # Downloaded attachment files
└── config.json        # Runtime config (account info, preferences)
```

### Configuration Priority

Environment variables > config.json > built-in defaults

## Development Phases

### Phase 1: Skeleton

- Init repo, copy crates, resolve compilation
- Axum server + static file serving
- Password auth + JWT middleware
- Config loading from environment

### Phase 2: Core Email

- Account management API (IMAP/SMTP config)
- OAuth flow adaptation (Gmail/Outlook)
- Background IMAP sync task
- Messages/folders/threads API
- Frontend: replace communication layer, inbox browsing

### Phase 3: Interactive Features

- Send email / drafts
- Attachment upload/download
- Read/star/archive/move/delete operations
- WebSocket realtime notifications
- Frontend: compose, attachments, flag operations

### Phase 4: Enhanced Features

- Full-text search API
- Translation
- Label management
- Folder unread counts

### Phase 5: Deployment & Release

- Dockerfile optimization (minimal image size)
- docker-compose template
- Health check endpoint (`GET /api/v1/health`)
- README and deployment guide
- GitHub Actions CI (build + push image)

## Non-Goals (for MVP)

- Multi-user support
- Mobile-responsive design (nice-to-have, not required)
- End-to-end encryption UI
- Calendar integration
- Plugin system
- Rules engine (pebble-rules crate — defer to post-MVP)
- Kanban view (defer to post-MVP)
