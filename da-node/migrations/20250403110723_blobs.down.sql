-- Add down migration script here

DROP TRIGGER IF EXISTS update_blobs_updated_at;
DROP INDEX IF EXISTS idx_blobs_hash;
DROP INDEX IF EXISTS idx_blobs_owner_id;
DROP TABLE IF EXISTS blobs;
