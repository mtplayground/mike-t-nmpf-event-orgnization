CREATE TABLE events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    host_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    slug TEXT NOT NULL,
    description_md TEXT NOT NULL DEFAULT '',
    start_at TIMESTAMPTZ NOT NULL,
    end_at TIMESTAMPTZ NOT NULL,
    timezone TEXT NOT NULL,
    location_type TEXT NOT NULL CHECK (location_type IN ('in_person', 'virtual', 'hybrid')),
    location_text TEXT,
    location_url TEXT,
    capacity INTEGER CHECK (capacity IS NULL OR capacity > 0),
    visibility TEXT NOT NULL CHECK (visibility IN ('draft', 'public', 'unlisted', 'private')),
    status TEXT NOT NULL CHECK (status IN ('draft', 'published', 'cancelled', 'completed')),
    cover_image_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    cancelled_at TIMESTAMPTZ,
    CHECK (end_at >= start_at),
    CHECK (status <> 'cancelled' OR cancelled_at IS NOT NULL)
);

CREATE UNIQUE INDEX events_slug_unique_idx ON events (slug);
CREATE INDEX events_host_id_idx ON events (host_id);
CREATE INDEX events_visibility_status_start_at_idx
    ON events (visibility, status, start_at);

ALTER TABLE event_images
    ADD CONSTRAINT event_images_event_id_fkey
    FOREIGN KEY (event_id)
    REFERENCES events (id)
    ON DELETE CASCADE;

ALTER TABLE events
    ADD CONSTRAINT events_cover_image_id_fkey
    FOREIGN KEY (cover_image_id)
    REFERENCES event_images (id)
    ON DELETE SET NULL;
