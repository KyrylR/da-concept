-- Add up migration script here

-- Create sync_status table to track blob synchronization across the network
CREATE TABLE sync_status
(
    blob_id              TEXT      NOT NULL,
    peer_node_id         TEXT      NOT NULL,
    sync_status          TEXT      NOT NULL CHECK (sync_status IN ('pending', 'completed', 'failed')),
    last_sync_attempt    TIMESTAMP,
    last_successful_sync TIMESTAMP,
    retry_count          INTEGER   NOT NULL DEFAULT 0,
    created_at           TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at           TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (blob_id, peer_node_id),
    FOREIGN KEY (blob_id) REFERENCES blobs (id) ON DELETE CASCADE
);

-- Create a trigger to automatically update the updated_at field
CREATE TRIGGER update_sync_status_updated_at
    AFTER UPDATE
    ON sync_status
    FOR EACH ROW
BEGIN
    UPDATE sync_status
    SET updated_at = CURRENT_TIMESTAMP
    WHERE blob_id = NEW.blob_id
      AND peer_node_id = NEW.peer_node_id;
END;
