-- Roles matrix, instance setup, registration mode, SSO providers, user prefs.

-- ---------------------------------------------------------------------------
-- Project roles: map legacy → new five-role matrix
-- ---------------------------------------------------------------------------
DO $$ BEGIN
    CREATE TYPE project_role_v2 AS ENUM (
        'admin',
        'test_architect',
        'test_automation_engineer',
        'developer',
        'viewer'
    );
EXCEPTION
    WHEN duplicate_object THEN NULL;
END $$;

ALTER TABLE memberships
    ALTER COLUMN role DROP DEFAULT;

ALTER TABLE memberships
    ALTER COLUMN role TYPE project_role_v2
    USING (
        CASE role::text
            WHEN 'admin' THEN 'admin'::project_role_v2
            WHEN 'project_lead' THEN 'test_architect'::project_role_v2
            WHEN 'tester' THEN 'test_automation_engineer'::project_role_v2
            WHEN 'viewer' THEN 'viewer'::project_role_v2
            ELSE 'viewer'::project_role_v2
        END
    );

DROP TYPE IF EXISTS project_role;
ALTER TYPE project_role_v2 RENAME TO project_role;

ALTER TABLE memberships
    ALTER COLUMN role SET DEFAULT 'viewer'::project_role;

-- ---------------------------------------------------------------------------
-- Users: force password change, locale/timezone overrides
-- ---------------------------------------------------------------------------
ALTER TABLE users
    ADD COLUMN IF NOT EXISTS must_change_password BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS locale TEXT,
    ADD COLUMN IF NOT EXISTS timezone TEXT;

-- ---------------------------------------------------------------------------
-- Instance / system settings (single-row style key-value)
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS system_settings (
    key TEXT PRIMARY KEY,
    value JSONB NOT NULL DEFAULT '{}'::jsonb,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO system_settings (key, value) VALUES
    ('setup_completed', 'false'::jsonb),
    ('instance_name', '"Vulyk"'::jsonb),
    ('base_url', '""'::jsonb),
    ('default_locale', '"en"'::jsonb),
    ('default_timezone', '"UTC"'::jsonb),
    ('registration_mode', '"disabled"'::jsonb),
    ('default_role', '"viewer"'::jsonb),
    ('allowed_email_domains', '[]'::jsonb)
ON CONFLICT (key) DO NOTHING;

-- One-time setup token (hash only); cleared when setup completes.
CREATE TABLE IF NOT EXISTS setup_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    token_hash TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    consumed_at TIMESTAMPTZ
);

-- ---------------------------------------------------------------------------
-- Invites (registration_mode = invite)
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS invites (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email TEXT NOT NULL,
    role project_role NOT NULL DEFAULT 'viewer',
    token_hash TEXT NOT NULL UNIQUE,
    invited_by UUID REFERENCES users (id) ON DELETE SET NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    accepted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS invites_email_idx ON invites (email);

-- ---------------------------------------------------------------------------
-- SSO / OIDC providers
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS sso_providers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider_key TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT FALSE,
    issuer_url TEXT,
    authorization_endpoint TEXT,
    token_endpoint TEXT,
    userinfo_endpoint TEXT,
    jwks_uri TEXT,
    client_id TEXT NOT NULL DEFAULT '',
    client_secret_ciphertext BYTEA,
    client_secret_nonce BYTEA,
    scopes TEXT NOT NULL DEFAULT 'openid email profile',
    extra_config JSONB NOT NULL DEFAULT '{}'::jsonb,
    default_role project_role NOT NULL DEFAULT 'viewer',
    group_role_mapping JSONB NOT NULL DEFAULT '{}'::jsonb,
    allowed_email_domains TEXT[] NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS user_identities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    provider_id UUID NOT NULL REFERENCES sso_providers (id) ON DELETE CASCADE,
    subject TEXT NOT NULL,
    email TEXT,
    raw_claims JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (provider_id, subject)
);

CREATE INDEX IF NOT EXISTS user_identities_user_id_idx ON user_identities (user_id);

CREATE TABLE IF NOT EXISTS oauth_states (
    state TEXT PRIMARY KEY,
    provider_id UUID NOT NULL REFERENCES sso_providers (id) ON DELETE CASCADE,
    nonce TEXT,
    redirect_to TEXT,
    code_verifier TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS oauth_states_expires_at_idx ON oauth_states (expires_at);

-- Seed known provider stubs (disabled until configured).
INSERT INTO sso_providers (provider_key, display_name, issuer_url, scopes, extra_config)
VALUES
    ('google', 'Google', 'https://accounts.google.com', 'openid email profile', '{}'::jsonb),
    ('microsoft', 'Microsoft Entra ID', NULL, 'openid email profile offline_access',
        '{"tenant":"common"}'::jsonb),
    ('github', 'GitHub', NULL, 'read:user user:email', '{}'::jsonb),
    ('gitlab', 'GitLab', NULL, 'openid email profile',
        '{"base_url":"https://gitlab.com"}'::jsonb),
    ('nextcloud', 'Nextcloud', NULL, 'openid email profile',
        '{"base_url":""}'::jsonb),
    ('oidc', 'Generic OIDC', NULL, 'openid email profile', '{}'::jsonb)
ON CONFLICT (provider_key) DO NOTHING;
