-- Add up migration script here

-- Create blobs table
CREATE TABLE blobs
(
    id           TEXT PRIMARY KEY CHECK (length(id) = 36), -- UUIDv4 as text
    content      BLOB      NOT NULL,
    metadata     TEXT,                                     -- Can be used to store JSON or other metadata
    content_type TEXT,
    size         INTEGER   NOT NULL,
    hash         TEXT,                                     -- For integrity verification
    owner_id     TEXT      NOT NULL,
    created_at   TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at   TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deleted_at   TIMESTAMP,
    FOREIGN KEY (owner_id) REFERENCES users (id) ON DELETE CASCADE
);

-- Create index on owner_id for faster lookups
CREATE INDEX idx_blobs_owner_id ON blobs (owner_id);

-- Create index on hash for faster lookups and deduplication
CREATE INDEX idx_blobs_hash ON blobs (hash);

-- Create a trigger to automatically update the updated_at field
CREATE TRIGGER update_blobs_updated_at
    AFTER UPDATE
    ON blobs
    FOR EACH ROW
BEGIN
    UPDATE blobs SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;
