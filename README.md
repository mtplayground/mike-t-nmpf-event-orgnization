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
