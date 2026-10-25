-- W2: case repository depth — tags, custom fields, versioning, attachments, FTS polish.

CREATE TABLE tags (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    color TEXT NOT NULL DEFAULT '#0f6b4c',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (project_id, name),
    CONSTRAINT tags_name_not_blank CHECK (length(trim(name)) > 0)
);

CREATE INDEX tags_project_id_idx ON tags (project_id);

CREATE TABLE case_tags (
    case_id UUID NOT NULL REFERENCES test_cases (id) ON DELETE CASCADE,
    tag_id UUID NOT NULL REFERENCES tags (id) ON DELETE CASCADE,
    PRIMARY KEY (case_id, tag_id)
);

CREATE INDEX case_tags_tag_id_idx ON case_tags (tag_id);

CREATE TABLE custom_fields (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    field_type TEXT NOT NULL DEFAULT 'text',
    options JSONB NOT NULL DEFAULT '[]'::jsonb,
    required BOOLEAN NOT NULL DEFAULT FALSE,
    sort_order INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (project_id, name),
    CONSTRAINT custom_fields_type_check CHECK (
        field_type IN ('text', 'number', 'boolean', 'select', 'multi_select', 'date')
    )
);

CREATE INDEX custom_fields_project_id_idx ON custom_fields (project_id);

CREATE TABLE custom_field_values (
    case_id UUID NOT NULL REFERENCES test_cases (id) ON DELETE CASCADE,
    field_id UUID NOT NULL REFERENCES custom_fields (id) ON DELETE CASCADE,
    value JSONB NOT NULL DEFAULT 'null'::jsonb,
    PRIMARY KEY (case_id, field_id)
);

CREATE INDEX custom_field_values_field_id_idx ON custom_field_values (field_id);

CREATE TABLE case_versions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id UUID NOT NULL REFERENCES test_cases (id) ON DELETE CASCADE,
    version INT NOT NULL,
    snapshot JSONB NOT NULL,
    change_summary TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (case_id, version)
);

CREATE INDEX case_versions_case_id_idx ON case_versions (case_id, version DESC);

CREATE TABLE attachments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    entity_type TEXT NOT NULL,
    entity_id UUID NOT NULL,
    filename TEXT NOT NULL,
    content_type TEXT NOT NULL DEFAULT 'application/octet-stream',
    size_bytes BIGINT NOT NULL DEFAULT 0,
    content BYTEA,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT attachments_entity_type_check CHECK (
        entity_type IN ('case', 'result', 'run')
    )
);

CREATE INDEX attachments_entity_idx ON attachments (entity_type, entity_id);
CREATE INDEX attachments_project_id_idx ON attachments (project_id);

-- Stronger FTS: include case key; fire on key changes too.
CREATE OR REPLACE FUNCTION test_cases_search_vector_update() RETURNS trigger AS $$
BEGIN
    NEW.search_vector :=
        setweight(to_tsvector('english', coalesce(NEW.key, '')), 'A') ||
        setweight(to_tsvector('english', coalesce(NEW.title, '')), 'A') ||
        setweight(to_tsvector('english', coalesce(NEW.description, '')), 'B') ||
        setweight(to_tsvector('english', coalesce(NEW.precondition, '')), 'C');
    NEW.updated_at := NOW();
    RETURN NEW;
END
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS test_cases_search_vector_trigger ON test_cases;
CREATE TRIGGER test_cases_search_vector_trigger
    BEFORE INSERT OR UPDATE OF key, title, description, precondition ON test_cases
    FOR EACH ROW EXECUTE FUNCTION test_cases_search_vector_update();

-- Backfill search vectors for existing rows (key was not previously indexed).
UPDATE test_cases SET title = title;

-- Composite index for paginated project listing.
CREATE INDEX IF NOT EXISTS test_cases_project_key_idx ON test_cases (project_id, key);
CREATE INDEX IF NOT EXISTS test_cases_project_updated_idx ON test_cases (project_id, updated_at DESC);
