CREATE TABLE design_components (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(), phase_id UUID NOT NULL REFERENCES phases(id) ON DELETE CASCADE,
 name VARCHAR(120) NOT NULL, category VARCHAR(20) NOT NULL CHECK(category IN ('FRONTEND','BACKEND','DATABASE','EXTERNAL')),
 responsibility TEXT NOT NULL, technology VARCHAR(120) NOT NULL, position INT NOT NULL DEFAULT 0,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(), updated_at TIMESTAMPTZ NOT NULL DEFAULT now(), UNIQUE(phase_id,name)
);
CREATE TABLE design_connections (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(), phase_id UUID NOT NULL REFERENCES phases(id) ON DELETE CASCADE,
 source_name VARCHAR(120) NOT NULL, target_name VARCHAR(120) NOT NULL, protocol VARCHAR(80) NOT NULL, description TEXT NOT NULL,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(), UNIQUE(phase_id,source_name,target_name,protocol)
);
CREATE TABLE design_documents (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(), phase_id UUID NOT NULL REFERENCES phases(id) ON DELETE CASCADE,
 kind VARCHAR(30) NOT NULL CHECK(kind IN ('UX','FEATURES','API_CONTRACTS','SECURITY','DECISIONS')),
 content JSONB NOT NULL DEFAULT '[]'::jsonb CHECK(jsonb_typeof(content)='array'), updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 UNIQUE(phase_id,kind)
);
CREATE INDEX design_components_phase_idx ON design_components(phase_id);
CREATE INDEX design_connections_phase_idx ON design_connections(phase_id);
CREATE INDEX design_documents_phase_idx ON design_documents(phase_id);
INSERT INTO validation_criteria(phase_id,code,label,required)
SELECT p.id,c.code,c.label,true FROM phases p CROSS JOIN (VALUES
 ('architecture_defined','Technical architecture defined'),('ux_flows_defined','UX flows defined'),
 ('features_specified','Features specified'),('api_contracts_defined','API contracts defined'),
 ('security_reviewed','Security reviewed'),('technology_decisions_accepted','Technology decisions accepted'),
 ('design_pack_ready','Design pack ready')) c(code,label)
WHERE p.phase_type='DESIGN' ON CONFLICT(phase_id,code) DO NOTHING;
