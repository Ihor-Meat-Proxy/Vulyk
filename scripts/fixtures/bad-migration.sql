-- Deliberately unsafe migration fixture for scripts/test-migration-lint.sh.
-- Not applied in production; Squawk must flag these patterns.
ALTER TABLE users DROP COLUMN email;
ALTER TABLE users ADD COLUMN required_field text NOT NULL;
CREATE INDEX users_email_idx ON users (email);
