-- Per-project test automation Git repositories (clone + index for LLM context).

CREATE TABLE automation_repos (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    provider TEXT NOT NULL DEFAULT 'generic',
    remote_url TEXT NOT NULL,
    branch TEXT NOT NULL DEFAULT 'main',
    subpath TEXT NOT NULL DEFAULT '',
    token_ciphertext BYTEA,
    token_nonce BYTEA,
    index_status TEXT NOT NULL DEFAULT 'never',
    last_indexed_at TIMESTAMPTZ,
    last_index_error TEXT,
    commit_sha TEXT,
    file_count INT NOT NULL DEFAULT 0,
    chunk_count INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT automation_repos_provider_check CHECK (
        provider IN ('github', 'gitlab', 'generic')
    ),
    CONSTRAINT automation_repos_status_check CHECK (
        index_status IN ('never', 'indexing', 'ready', 'error')
    ),
    CONSTRAINT automation_repos_project_unique UNIQUE (project_id)
);

CREATE TABLE automation_repo_files (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    repo_id UUID NOT NULL REFERENCES automation_repos (id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    language TEXT NOT NULL DEFAULT '',
    size_bytes INT NOT NULL DEFAULT 0,
    content_hash TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT automation_repo_files_unique UNIQUE (repo_id, path)
);

CREATE INDEX automation_repo_files_repo_idx ON automation_repo_files (repo_id);

CREATE TABLE automation_repo_chunks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    file_id UUID NOT NULL REFERENCES automation_repo_files (id) ON DELETE CASCADE,
    chunk_index INT NOT NULL,
    content TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT automation_repo_chunks_unique UNIQUE (file_id, chunk_index)
);

CREATE INDEX automation_repo_chunks_file_idx ON automation_repo_chunks (file_id);
