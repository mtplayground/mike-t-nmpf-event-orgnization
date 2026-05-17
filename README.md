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
cp .env.example .env
cd backend
cargo build
```

### Frontend

```bash
cd frontend
npm install
npm run dev -- --host 0.0.0.0 --port 8080
```
