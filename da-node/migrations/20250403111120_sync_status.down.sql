-- Add down migration script here

DROP TRIGGER IF EXISTS update_sync_status_updated_at;
DROP TABLE IF EXISTS sync_status;