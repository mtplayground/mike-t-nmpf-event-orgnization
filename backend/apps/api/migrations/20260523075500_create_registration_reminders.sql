CREATE TABLE registration_reminders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    registration_id UUID NOT NULL REFERENCES registrations (id) ON DELETE CASCADE,
    event_id UUID NOT NULL REFERENCES events (id) ON DELETE CASCADE,
    reminder_kind TEXT NOT NULL CHECK (reminder_kind IN ('24h')),
    enqueued_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX registration_reminders_registration_id_kind_unique_idx
    ON registration_reminders (registration_id, reminder_kind);
CREATE INDEX registration_reminders_event_id_idx
    ON registration_reminders (event_id);
