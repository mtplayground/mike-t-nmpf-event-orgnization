# Mike T NMPF Event Organization

Initial repository structure for the event organization platform.

## Repository layout

- `backend/`: Rust Cargo workspace for the API and shared backend crates.
- `frontend/`: Vite React + TypeScript application.

## Tooling

- Formatting:
  - Rust: `cargo fmt --all --check`
  - Frontend: `npm run format:check`
- Linting:
  - Rust: `cargo clippy --workspace --all-targets -- -D warnings`
  - Frontend: `npm run lint`
- Build:
  - Backend: `cd backend && cargo build`
  - Frontend: `cd frontend && npm run build`

## Getting started

### Backend

```bash
export PATH=/usr/local/cargo/bin:$PATH
export DATABASE_URL=$(cat /workspace/.database_url)
export DATABASE_SSL_MODE=prefer
export HOST=0.0.0.0
export PORT=8080
export DATABASE_MIN_CONNECTIONS=1
export DATABASE_MAX_CONNECTIONS=10
export DATABASE_ACQUIRE_TIMEOUT_SECONDS=10
export DATABASE_IDLE_TIMEOUT_SECONDS=600
export DATABASE_MAX_LIFETIME_SECONDS=1800
export JWT_ACCESS_SECRET=replace-with-a-long-random-access-secret
export JWT_REFRESH_SECRET=replace-with-a-long-random-refresh-secret
export SMTP_HOST=smtp.example.com
export SMTP_USERNAME=smtp-user
export SMTP_PASSWORD=replace-with-smtp-password
export SMTP_FROM_EMAIL=no-reply@example.com
export OBJECT_STORAGE_ENDPOINT=https://s3.example.com
export OBJECT_STORAGE_BUCKET=event-assets
export OBJECT_STORAGE_ACCESS_KEY_ID=replace-with-access-key-id
export OBJECT_STORAGE_SECRET_ACCESS_KEY=replace-with-secret-access-key
cd backend
cargo build
```

### Database migrations

```bash
export PATH=/usr/local/cargo/bin:$PATH
export DATABASE_URL=$(cat /workspace/.database_url)
cd backend
cargo sqlx migrate run --source apps/api/migrations
```

The API also runs embedded migrations on startup via `sqlx::migrate!()`.

### SQLx prepare workflow

```bash
export PATH=/usr/local/cargo/bin:$PATH
export DATABASE_URL=$(cat /workspace/.database_url)
cd backend
cargo sqlx prepare --workspace
```

Commit the generated `.sqlx/` metadata alongside future query changes that rely on SQLx compile-time verification.

### Object storage wrapper

The backend includes an S3-compatible client wrapper in `backend/apps/api/src/object_storage.rs`.
It uses the existing object storage environment variables and exposes helpers for presigned PUT/GET URLs plus `head_object` and `delete_object`.

### Frontend

```bash
cd frontend
npm install
export VITE_API_BASE_URL=http://127.0.0.1:8080
npm run dev -- --host 0.0.0.0 --port 8080
```

## Deployment

This repository ships script-based deployment packaging only. There is no
Dockerfile and no CI deployment workflow.

### Build release artifacts

Build the backend release binary into `dist/backend/event-organization-api`:

```bash
./scripts/build-backend-release.sh
```

Build the frontend static bundle into `dist/frontend`:

```bash
export VITE_API_BASE_URL=https://events.example.com/api
./scripts/build-frontend-bundle.sh
```

Set `TARGET_DIR` to write either artifact somewhere else:

```bash
TARGET_DIR=/opt/event-organization/backend ./scripts/build-backend-release.sh
TARGET_DIR=/var/www/event-organization/frontend ./scripts/build-frontend-bundle.sh
```

### Runtime checklist

1. Provision PostgreSQL and set the production `DATABASE_URL`.
2. Configure all keys listed in `.env.example`; use `deploy/ENVIRONMENT.md` as
   the production checklist.
3. Run migrations before starting a new release:

   ```bash
   cd backend
   cargo sqlx migrate run --source apps/api/migrations
   ```

   The API also runs embedded migrations on startup, but an explicit migration
   step makes deploy failures easier to diagnose.
4. Install the backend binary on the API host and run it with the production
   environment loaded.
5. Copy the frontend bundle to the web root used by nginx.
6. Adapt `deploy/nginx/event-organization.conf` for the public hostname and
   release paths, then reload nginx.
7. Verify:
   - `GET /api/health` returns `{ "data": { "status": "ok" } }`.
   - The frontend loads from the public hostname.
   - Registration email, object upload, and event cover processing work against
     the production SMTP and object storage providers.

The sample nginx config serves the Vite single-page app and proxies `/api/` to
the Rust API on `127.0.0.1:8080`. When using that config, build the frontend
with `VITE_API_BASE_URL` set to the public `/api` URL.
