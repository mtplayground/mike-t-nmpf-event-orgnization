CREATE TABLE event_images (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id UUID NOT NULL,
    object_key TEXT NOT NULL,
    variant TEXT NOT NULL CHECK (variant IN ('hero', 'thumbnail')),
    width INTEGER NOT NULL CHECK (width > 0),
    height INTEGER NOT NULL CHECK (height > 0),
    bytes BIGINT NOT NULL CHECK (bytes > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX event_images_event_id_variant_unique_idx
    ON event_images (event_id, variant);
CREATE UNIQUE INDEX event_images_object_key_unique_idx
    ON event_images (object_key);
CREATE INDEX event_images_event_id_idx
    ON event_images (event_id);
