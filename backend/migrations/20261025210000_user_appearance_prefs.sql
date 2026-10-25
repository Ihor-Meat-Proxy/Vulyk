-- Per-user appearance preferences (theme, font size, density, reduced motion).
ALTER TABLE users
    ADD COLUMN IF NOT EXISTS preferences JSONB NOT NULL DEFAULT '{}'::jsonb;

COMMENT ON COLUMN users.preferences IS
    'Appearance prefs: theme, font_size, density, reduced_motion';

-- Instance default theme (system | light | dark | high_contrast | sepia).
INSERT INTO system_settings (key, value)
VALUES ('default_theme', '"system"'::jsonb)
ON CONFLICT (key) DO NOTHING;
