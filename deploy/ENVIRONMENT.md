# Deployment Environment Checklist

Use `.env.example` as the source list for supported keys. Keep production
secrets outside git and load them through the process manager or host secret
store.

## Backend API

- `HOST` and `PORT` bind the Rust API. The sample nginx config expects
  `127.0.0.1:8080`.
- `DATABASE_URL` must point at the application PostgreSQL database.
- `DATABASE_SSL_MODE` should match the database provider requirement.
- `DATABASE_MIN_CONNECTIONS`, `DATABASE_MAX_CONNECTIONS`,
  `DATABASE_ACQUIRE_TIMEOUT_SECONDS`, `DATABASE_IDLE_TIMEOUT_SECONDS`, and
  `DATABASE_MAX_LIFETIME_SECONDS` tune the SQLx pool.
- `JWT_ACCESS_SECRET` and `JWT_REFRESH_SECRET` must be long independent random
  values.
- `JWT_ISSUER`, `JWT_ACCESS_TTL_SECONDS`, and `JWT_REFRESH_TTL_SECONDS` should
  be explicit in production.
- `SMTP_HOST`, `SMTP_PORT`, `SMTP_USERNAME`, `SMTP_PASSWORD`,
  `SMTP_FROM_EMAIL`, `SMTP_FROM_NAME`, and `SMTP_USE_STARTTLS` must be valid
  for transactional email.
- `OBJECT_STORAGE_ENDPOINT`, `OBJECT_STORAGE_REGION`, `OBJECT_STORAGE_BUCKET`,
  `OBJECT_STORAGE_ACCESS_KEY_ID`, `OBJECT_STORAGE_SECRET_ACCESS_KEY`, and
  `OBJECT_STORAGE_PUBLIC_BASE_URL` must point at an S3-compatible bucket.
- `RUST_LOG=info,tower_http=info` is a practical baseline for structured
  request logs.

## Frontend Build

- `VITE_API_BASE_URL` is baked into the static bundle at build time.
- For the sample nginx config, use `VITE_API_BASE_URL=https://events.example.com/api`.
- Rebuild the frontend whenever the public API origin changes.

## External Service Checks

- PostgreSQL accepts connections from the API host and has migrations applied.
- SMTP credentials can send verification, reset, confirmation, reminder, and
  announcement emails.
- Object storage permits presigned PUT/HEAD/GET/DELETE operations from the API.
- Object storage CORS allows browser PUT uploads from the deployed frontend
  origin.
- `OBJECT_STORAGE_PUBLIC_BASE_URL` serves derived event images and avatars to
  browsers.
