# Product Contract

## Project

Mike T NMPF Event Organization is an event organization platform with a Rust API backend and a Vite React frontend. The current product foundation supports authenticated users, host and attendee application areas, event records, and event image upload/processing infrastructure.

## Current Capabilities

- User accounts: registration, login, refresh/logout, email verification, forgot/reset password, and `/auth/me`.
- Profiles: authenticated users can read and update display name, bio, and avatar object key.
- Avatars: backend can issue presigned upload URLs and confirm uploaded avatar objects against S3-compatible object storage metadata.
- Events: hosts can create, read, update, cancel, and duplicate their own events through authenticated host-scoped endpoints.
- Event fields: title, generated unique slug, markdown description, start/end time, timezone, location type/details, capacity, visibility, status, and optional cover image reference.
- Event cover images: backend issues presigned upload URLs for owned events, validates uploaded image content, generates hero and thumbnail PNG variants, stores image metadata, and cleans up replaced variants.
- Frontend: React Router app shell with public auth pages plus protected profile, host, and attendee pages; API client supports JSON requests and upload progress.

## Architecture

- Backend is a Rust Cargo workspace with an Axum HTTP API, SQLx/PostgreSQL persistence, JWT auth, SMTP email delivery, and an S3-compatible object storage wrapper.
- Database migrations are embedded and also runnable from `backend/apps/api/migrations`.
- API responses use `{ "data": ... }`; errors use `{ "error": { "code", "message", "fields?" } }`.
- Authenticated routes use bearer access tokens and a current-user extractor.
- Host event ownership is enforced by filtering event reads/writes by both event id and authenticated host id.
- Object uploads use presigned URLs; the API confirms uploaded objects before persisting references or derived assets.

## Conventions

- Default branch is `main`.
- Backend code lives under `backend/apps/api`; frontend code lives under `frontend/src`.
- Rust validation uses request DTOs plus `validator`; business validation returns structured API errors.
- Event enum wire values are snake_case: `in_person`, `virtual`, `hybrid`, `draft`, `public`, `unlisted`, `private`, `published`, `cancelled`, `completed`.
- Recommended verification commands are `cargo build`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings` from `backend`.
