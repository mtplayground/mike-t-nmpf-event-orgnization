# Product Contract

## Project

Mike T NMPF Event Organization is an event organization platform with a Rust API backend and a Vite React frontend. The current product supports authenticated users, host and attendee application areas, event publishing and discovery, registration workflows, attendee communications, reminder emails, and event image upload/processing infrastructure.

## Current Capabilities

- User accounts: registration, login, refresh/logout, email verification, forgot/reset password, and `/auth/me`.
- Profiles: authenticated users can read and update display name, bio, and avatar object key.
- Avatars: backend can issue presigned upload URLs and confirm uploaded avatar objects against S3-compatible object storage metadata.
- Events: hosts can create, read, update, cancel, duplicate, and list their own events through authenticated host-scoped endpoints.
- Event fields: title, generated unique slug, markdown description, start/end time, timezone, location type/details, capacity, visibility, status, and optional cover image reference.
- Host event lists: `GET /me/events?status=draft|upcoming|past` returns paginated event rows with attendee counts and pagination metadata.
- Public discovery: `GET /events` lists upcoming public published events with search/date/cursor filters and thumbnail metadata; `GET /events/{slug}` returns event detail, minimal host profile, attendee count, capacity remaining, and the authenticated user's registration state when available.
- Event cover images: backend issues presigned upload URLs for owned events, validates uploaded image content, generates hero and thumbnail PNG variants, stores image metadata, and cleans up replaced variants.
- Registrations: backend has a `registrations` table and repository model with unique `(event_id, user_id)`, `registered`/`cancelled` states, row-lock-based capacity checks, cancellation, and re-registration support.
- Registration email flows: attendee registration confirmation emails include calendar attachment content, and a background worker queues 24-hour event reminder emails for due active registrations.
- Host attendee operations: hosts can list event attendees, download attendee CSV, and queue rate-limited announcement emails to currently registered attendees for their own events.
- Attendee area: authenticated attendees can browse public events, register/cancel from event detail, and view upcoming/past registrations with pagination.
- Frontend: React Router app shell with public auth pages, protected profile/host/attendee areas, host event dashboard, event create/edit form with cover upload, attendee discovery feed, public event detail page, attendee registration dashboard, and host attendee management page with CSV export and announcement composer.
- Testing: frontend includes a Playwright E2E suite covering registration, verification, event creation with cover, publishing, attendee registration/cancellation, host CSV export, and announcements.

## Architecture

- Backend is a Rust Cargo workspace with an Axum HTTP API, SQLx/PostgreSQL persistence, JWT auth, SMTP email delivery, and an S3-compatible object storage wrapper.
- Database migrations are embedded and also runnable from `backend/apps/api/migrations`.
- API responses use `{ "data": ... }`; errors use `{ "error": { "code", "message", "fields?" } }`.
- Authenticated routes use bearer access tokens and a current-user extractor.
- Host event ownership is enforced by filtering event reads/writes, attendee exports, and announcement sends by authenticated host id.
- Object uploads use presigned URLs; the API confirms uploaded objects before persisting references or derived assets.
- Registration capacity enforcement locks the target event row before counting active registrations.
- HTTP requests receive or reuse an `x-request-id`; structured tracing records request id, method, path, status, latency, and authenticated user id when available.
- Backend error handling maps validation, JSON, auth, conflict, not-found, rate-limit, SQL row-not-found, and SQL unique-violation failures into consistent API error envelopes.

## Conventions

- Default branch is `main`.
- Backend code lives under `backend/apps/api`; frontend code lives under `frontend/src`.
- Rust validation uses request DTOs plus `validator`; business validation returns structured API errors.
- Event enum wire values are snake_case: `in_person`, `virtual`, `hybrid`, `draft`, `public`, `unlisted`, `private`, `published`, `cancelled`, `completed`.
- Registration status wire values are snake_case: `registered`, `cancelled`.
- Recommended verification commands are `cargo build`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings` from `backend`.
- Frontend verification commands include `npm run build`, `npm run lint`, `npm run format:check`, and `npm run test:e2e` from `frontend`.
