-- W9/W10: LLM provider configs (encrypted keys) + API tokens for MCP auth.
-- Minimal run/result helpers already exist in 002; tokens are intentionally
-- lean so W1 can attach user_id / scopes without a rewrite.

CREATE TABLE llm_configs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID REFERENCES projects (id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    name TEXT NOT NULL,
    base_url TEXT,
    model TEXT NOT NULL DEFAULT '',
    api_key_ciphertext BYTEA,
    api_key_nonce BYTEA,
    extra JSONB NOT NULL DEFAULT '{}'::jsonb,
    is_default BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT llm_configs_provider_check CHECK (
        provider IN (
            'ollama',
            'openai_compatible',
            'openrouter',
            'anthropic',
            'gemini',
            'openai',
            'xai',
            'mock'
        )
    )
);

CREATE INDEX llm_configs_project_id_idx ON llm_configs (project_id);
CREATE UNIQUE INDEX llm_configs_default_per_project_idx
    ON llm_configs (COALESCE(project_id, '00000000-0000-0000-0000-000000000000'))
    WHERE is_default;

CREATE TABLE api_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    token_prefix TEXT NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ
);

CREATE INDEX api_tokens_prefix_idx ON api_tokens (token_prefix);
