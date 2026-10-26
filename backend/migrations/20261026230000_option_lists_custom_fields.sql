-- Admin option lists + global custom field definitions (cases & requirements).
-- Evolves legacy per-project custom_fields into reusable definitions with scope.

CREATE TABLE option_lists (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    key TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT option_lists_key_not_blank CHECK (length(trim(key)) > 0),
    CONSTRAINT option_lists_name_not_blank CHECK (length(trim(name)) > 0),
    CONSTRAINT option_lists_key_unique UNIQUE (key)
);

CREATE TABLE option_list_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    list_id UUID NOT NULL REFERENCES option_lists (id) ON DELETE CASCADE,
    key TEXT,
    label TEXT NOT NULL,
    value TEXT,
    colour TEXT,
    description TEXT NOT NULL DEFAULT '',
    sort_order INT NOT NULL DEFAULT 0,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    deprecated BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT option_list_items_label_not_blank CHECK (length(trim(label)) > 0)
);

CREATE UNIQUE INDEX option_list_items_list_key_uidx
    ON option_list_items (list_id, key)
    WHERE key IS NOT NULL AND length(trim(key)) > 0;

CREATE INDEX option_list_items_list_sort_idx
    ON option_list_items (list_id, sort_order, label);

CREATE TABLE custom_field_definitions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    key TEXT NOT NULL,
    name TEXT NOT NULL,
    field_type TEXT NOT NULL DEFAULT 'text',
    entity TEXT NOT NULL DEFAULT 'test_case',
    option_list_id UUID REFERENCES option_lists (id) ON DELETE RESTRICT,
    default_value JSONB NOT NULL DEFAULT 'null'::jsonb,
    required BOOLEAN NOT NULL DEFAULT FALSE,
    help_text TEXT NOT NULL DEFAULT '',
    sort_order INT NOT NULL DEFAULT 0,
    show_in_list BOOLEAN NOT NULL DEFAULT FALSE,
    show_in_filters BOOLEAN NOT NULL DEFAULT FALSE,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    scope TEXT NOT NULL DEFAULT 'all',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT custom_field_definitions_key_not_blank CHECK (length(trim(key)) > 0),
    CONSTRAINT custom_field_definitions_name_not_blank CHECK (length(trim(name)) > 0),
    CONSTRAINT custom_field_definitions_key_unique UNIQUE (key),
    CONSTRAINT custom_field_definitions_entity_check CHECK (
        entity IN ('test_case', 'requirement', 'both')
    ),
    CONSTRAINT custom_field_definitions_scope_check CHECK (
        scope IN ('all', 'selected')
    ),
    CONSTRAINT custom_field_definitions_type_check CHECK (
        field_type IN (
            'text', 'long_text', 'number', 'checkbox', 'boolean',
            'dropdown', 'select', 'multi_select', 'date', 'url'
        )
    )
);

CREATE INDEX custom_field_definitions_active_sort_idx
    ON custom_field_definitions (active, sort_order, name);

CREATE TABLE custom_field_projects (
    field_id UUID NOT NULL REFERENCES custom_field_definitions (id) ON DELETE CASCADE,
    project_id UUID NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    required_override BOOLEAN,
    default_value_override JSONB,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (field_id, project_id)
);

CREATE INDEX custom_field_projects_project_idx ON custom_field_projects (project_id);

-- Migrate legacy per-project custom_fields into definitions + project links.
ALTER TABLE custom_field_values RENAME TO custom_field_values_legacy;
-- Indexes keep their names after RENAME TABLE; drop so we can recreate on the new table.
DROP INDEX IF EXISTS custom_field_values_field_id_idx;

CREATE TABLE custom_field_values (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    field_id UUID NOT NULL REFERENCES custom_field_definitions (id) ON DELETE RESTRICT,
    case_id UUID REFERENCES test_cases (id) ON DELETE CASCADE,
    requirement_id UUID REFERENCES requirements (id) ON DELETE CASCADE,
    value JSONB NOT NULL DEFAULT 'null'::jsonb,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT custom_field_values_one_entity CHECK (
        (CASE WHEN case_id IS NOT NULL THEN 1 ELSE 0 END)
        + (CASE WHEN requirement_id IS NOT NULL THEN 1 ELSE 0 END) = 1
    )
);

CREATE UNIQUE INDEX custom_field_values_case_field_uidx
    ON custom_field_values (case_id, field_id)
    WHERE case_id IS NOT NULL;

CREATE UNIQUE INDEX custom_field_values_req_field_uidx
    ON custom_field_values (requirement_id, field_id)
    WHERE requirement_id IS NOT NULL;

CREATE INDEX custom_field_values_field_id_idx ON custom_field_values (field_id);
CREATE INDEX custom_field_values_value_gin ON custom_field_values USING GIN (value jsonb_path_ops);
CREATE INDEX custom_field_values_field_scalar_idx
    ON custom_field_values (field_id, (value #>> '{}'));

DO $$
DECLARE
    r RECORD;
    new_id UUID;
    list_id UUID;
    base_key TEXT;
    final_key TEXT;
    list_key TEXT;
    suffix INT;
    mapped_type TEXT;
    opt JSONB;
    opt_label TEXT;
    opt_key TEXT;
    opt_idx INT;
BEGIN
    FOR r IN SELECT * FROM custom_fields ORDER BY created_at, id LOOP
        mapped_type := CASE r.field_type
            WHEN 'boolean' THEN 'checkbox'
            WHEN 'select' THEN 'dropdown'
            ELSE r.field_type
        END;

        base_key := lower(regexp_replace(trim(r.name), '[^a-zA-Z0-9]+', '_', 'g'));
        base_key := trim(both '_' from base_key);
        IF base_key IS NULL OR base_key = '' THEN
            base_key := 'field';
        END IF;
        final_key := base_key;
        suffix := 1;
        WHILE EXISTS (SELECT 1 FROM custom_field_definitions WHERE key = final_key) LOOP
            suffix := suffix + 1;
            final_key := base_key || '_' || suffix::text;
        END LOOP;

        list_id := NULL;
        IF mapped_type IN ('dropdown', 'multi_select') THEN
            list_key := final_key || '_options';
            suffix := 1;
            WHILE EXISTS (SELECT 1 FROM option_lists WHERE key = list_key) LOOP
                suffix := suffix + 1;
                list_key := final_key || '_options_' || suffix::text;
            END LOOP;

            list_id := gen_random_uuid();
            INSERT INTO option_lists (id, key, name, description)
            VALUES (list_id, list_key, r.name || ' options', 'Migrated from legacy custom field options');

            opt_idx := 0;
            IF jsonb_typeof(r.options) = 'array' THEN
                FOR opt IN SELECT * FROM jsonb_array_elements(r.options) LOOP
                    IF jsonb_typeof(opt) = 'string' THEN
                        opt_label := opt #>> '{}';
                        opt_key := lower(regexp_replace(opt_label, '[^a-zA-Z0-9]+', '_', 'g'));
                    ELSIF jsonb_typeof(opt) = 'object' THEN
                        opt_label := COALESCE(opt->>'label', opt->>'name', opt->>'value', opt->>'key', '');
                        opt_key := COALESCE(NULLIF(opt->>'key', ''), NULLIF(opt->>'value', ''), opt_label);
                        opt_key := lower(regexp_replace(opt_key, '[^a-zA-Z0-9]+', '_', 'g'));
                    ELSE
                        opt_label := opt::text;
                        opt_key := 'opt_' || opt_idx::text;
                    END IF;
                    IF opt_label IS NULL OR trim(opt_label) = '' THEN
                        opt_label := 'Option ' || (opt_idx + 1)::text;
                    END IF;
                    INSERT INTO option_list_items (list_id, key, label, value, sort_order)
                    VALUES (
                        list_id,
                        NULLIF(trim(both '_' from opt_key), ''),
                        opt_label,
                        opt_label,
                        opt_idx
                    );
                    opt_idx := opt_idx + 1;
                END LOOP;
            END IF;
        END IF;

        new_id := gen_random_uuid();
        INSERT INTO custom_field_definitions (
            id, key, name, field_type, entity, option_list_id, default_value,
            required, help_text, sort_order, show_in_list, show_in_filters,
            active, scope, created_at, updated_at
        ) VALUES (
            new_id, final_key, r.name, mapped_type, 'test_case', list_id, 'null'::jsonb,
            r.required, '', r.sort_order, FALSE, FALSE,
            TRUE, 'selected', r.created_at, r.created_at
        );

        INSERT INTO custom_field_projects (field_id, project_id, enabled)
        VALUES (new_id, r.project_id, TRUE);

        INSERT INTO custom_field_values (field_id, case_id, value)
        SELECT new_id, cfv.case_id, cfv.value
        FROM custom_field_values_legacy cfv
        WHERE cfv.field_id = r.id;
    END LOOP;
END $$;

DROP TABLE custom_field_values_legacy;
DROP TABLE custom_fields;

-- Enforce list requirement after migration.
ALTER TABLE custom_field_definitions
    ADD CONSTRAINT custom_field_definitions_list_required CHECK (
        (field_type IN ('dropdown', 'select', 'multi_select') AND option_list_id IS NOT NULL)
        OR (field_type NOT IN ('dropdown', 'select', 'multi_select'))
    );
