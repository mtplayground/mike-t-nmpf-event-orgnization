CREATE TABLE registrations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id UUID NOT NULL REFERENCES events (id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK (status IN ('registered', 'cancelled')),
    registered_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    cancelled_at TIMESTAMPTZ,
    CHECK (status <> 'registered' OR cancelled_at IS NULL),
    CHECK (status <> 'cancelled' OR cancelled_at IS NOT NULL)
);

CREATE UNIQUE INDEX registrations_event_id_user_id_unique_idx
    ON registrations (event_id, user_id);
CREATE INDEX registrations_event_id_status_idx
    ON registrations (event_id, status);
CREATE INDEX registrations_user_id_status_idx
    ON registrations (user_id, status);
