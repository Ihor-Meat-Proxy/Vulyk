-- Requirement folders + acceptance criteria; chat creation audit trail.

CREATE TABLE requirement_folders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    parent_id UUID REFERENCES requirement_folders (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    sort_order INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX requirement_folders_project_idx ON requirement_folders (project_id);
CREATE INDEX requirement_folders_parent_idx ON requirement_folders (parent_id);

ALTER TABLE requirements
    ADD COLUMN IF NOT EXISTS folder_id UUID REFERENCES requirement_folders (id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS acceptance_criteria TEXT NOT NULL DEFAULT '';

CREATE INDEX requirements_folder_idx ON requirements (folder_id);

CREATE TABLE chat_creation_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    user_id UUID REFERENCES users (id) ON DELETE SET NULL,
    provider TEXT NOT NULL DEFAULT '',
    model TEXT NOT NULL DEFAULT '',
    kind TEXT NOT NULL,
    entity_id UUID,
    entity_key TEXT,
    summary TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chat_creation_events_kind_check CHECK (
        kind IN (
            'case',
            'requirement',
            'suite',
            'requirement_folder',
            'case_requirement_link'
        )
    )
);

CREATE INDEX chat_creation_events_project_idx
    ON chat_creation_events (project_id, created_at DESC);
