-- Add down migration script here

DROP TRIGGER IF EXISTS update_users_updated_at;
DROP TABLE IF EXISTS users;