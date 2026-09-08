CREATE TABLE model_entities (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(), phase_id UUID NOT NULL REFERENCES phases(id) ON DELETE CASCADE,
 conceptual_name VARCHAR(120) NOT NULL, logical_name VARCHAR(120), physical_name VARCHAR(120), description TEXT,
 position INT NOT NULL DEFAULT 0, created_at TIMESTAMPTZ NOT NULL DEFAULT now(), updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 UNIQUE(phase_id, conceptual_name)
);
CREATE TABLE model_attributes (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(), entity_id UUID NOT NULL REFERENCES model_entities(id) ON DELETE CASCADE,
 conceptual_name VARCHAR(120) NOT NULL, logical_name VARCHAR(120), physical_name VARCHAR(120), data_type VARCHAR(80),
 is_primary_key BOOLEAN NOT NULL DEFAULT false, is_unique BOOLEAN NOT NULL DEFAULT false, is_nullable BOOLEAN NOT NULL DEFAULT true,
 default_value TEXT, position INT NOT NULL DEFAULT 0, created_at TIMESTAMPTZ NOT NULL DEFAULT now(), updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 UNIQUE(entity_id, conceptual_name)
);
CREATE TABLE model_relationships (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(), phase_id UUID NOT NULL REFERENCES phases(id) ON DELETE CASCADE,
 name VARCHAR(120) NOT NULL, source_entity_id UUID NOT NULL REFERENCES model_entities(id) ON DELETE CASCADE,
 target_entity_id UUID NOT NULL REFERENCES model_entities(id) ON DELETE CASCADE,
 source_cardinality VARCHAR(4) NOT NULL CHECK(source_cardinality IN ('0..1','1','0..N','1..N')),
 target_cardinality VARCHAR(4) NOT NULL CHECK(target_cardinality IN ('0..1','1','0..N','1..N')),
 description TEXT, created_at TIMESTAMPTZ NOT NULL DEFAULT now(), updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 UNIQUE(phase_id, name)
);
CREATE TABLE business_rules (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(), phase_id UUID NOT NULL REFERENCES phases(id) ON DELETE CASCADE,
 code VARCHAR(30) NOT NULL, title VARCHAR(180) NOT NULL, description TEXT NOT NULL,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(), updated_at TIMESTAMPTZ NOT NULL DEFAULT now(), UNIQUE(phase_id, code)
);
CREATE INDEX model_entities_phase_idx ON model_entities(phase_id);
CREATE INDEX model_attributes_entity_idx ON model_attributes(entity_id);
CREATE INDEX model_relationships_phase_idx ON model_relationships(phase_id);
CREATE INDEX model_relationships_source_idx ON model_relationships(source_entity_id);
CREATE INDEX model_relationships_target_idx ON model_relationships(target_entity_id);
CREATE INDEX business_rules_phase_idx ON business_rules(phase_id);
INSERT INTO validation_criteria (phase_id, code, label, required)
SELECT p.id, criterion.code, criterion.label, true FROM phases p CROSS JOIN (VALUES
 ('entities_defined', 'Entities defined'), ('relations_defined', 'Relationships defined'),
 ('business_rules_defined', 'Business rules defined'), ('mcd_defined', 'MCD ready'),
 ('mld_defined', 'MLD ready'), ('mpd_defined', 'MPD ready')
) AS criterion(code, label) WHERE p.phase_type = 'MODEL'
ON CONFLICT (phase_id, code) DO NOTHING;
