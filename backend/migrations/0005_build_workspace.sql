CREATE TABLE build_profiles (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
 phase_id UUID NOT NULL UNIQUE REFERENCES phases(id) ON DELETE CASCADE,
 repository_url TEXT NOT NULL DEFAULT '',
 default_branch VARCHAR(120) NOT NULL DEFAULT 'main',
 integration_strategy VARCHAR(30) NOT NULL DEFAULT 'PULL_REQUEST' CHECK(integration_strategy IN ('PULL_REQUEST','TRUNK_BASED','GIT_FLOW')),
 ci_required BOOLEAN NOT NULL DEFAULT true,
 definition_of_done JSONB NOT NULL DEFAULT '[]'::jsonb CHECK(jsonb_typeof(definition_of_done)='array'),
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO validation_criteria(phase_id,code,label,required)
SELECT p.id,c.code,c.label,true FROM phases p CROSS JOIN (VALUES
 ('design_pack_available','Validated Design Pack available'),
 ('backlog_ready','Implementation backlog ready'),
 ('requirements_linked','Tasks linked to requirements'),
 ('execution_decisions_recorded','Execution decisions recorded'),
 ('github_ready','GitHub preparation complete'),
 ('build_plan_ready','Build plan ready')) c(code,label)
WHERE p.phase_type='BUILD' ON CONFLICT(phase_id,code) DO NOTHING;
