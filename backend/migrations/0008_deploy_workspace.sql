CREATE TABLE deploy_profiles (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
 phase_id UUID NOT NULL UNIQUE REFERENCES phases(id) ON DELETE CASCADE,
 target_environment VARCHAR(120) NOT NULL DEFAULT '',
 production_url TEXT NOT NULL DEFAULT '',
 provider VARCHAR(160) NOT NULL DEFAULT '',
 deployment_status VARCHAR(20) NOT NULL DEFAULT 'PREPARING' CHECK(deployment_status IN ('PREPARING','READY','DEPLOYED','FAILED','ROLLED_BACK')),
 configuration_notes TEXT NOT NULL DEFAULT '',
 configuration_keys JSONB NOT NULL DEFAULT '[]'::jsonb CHECK(jsonb_typeof(configuration_keys)='array'),
 migrations_required BOOLEAN NOT NULL DEFAULT false,
 migrations_plan TEXT NOT NULL DEFAULT '',
 backups_required BOOLEAN NOT NULL DEFAULT false,
 backups_plan TEXT NOT NULL DEFAULT '',
 monitoring_required BOOLEAN NOT NULL DEFAULT true,
 monitoring_plan TEXT NOT NULL DEFAULT '',
 healthcheck_required BOOLEAN NOT NULL DEFAULT true,
 healthcheck TEXT NOT NULL DEFAULT '',
 rollback_strategy TEXT NOT NULL DEFAULT '',
 deployed_version VARCHAR(120) NOT NULL DEFAULT '',
 deployed_at TIMESTAMPTZ,
 release_notes TEXT NOT NULL DEFAULT '',
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO validation_criteria(phase_id,code,label,required)
SELECT p.id,c.code,c.label,true FROM phases p CROSS JOIN (VALUES
 ('test_report_available','Validated Test Report available'),
 ('production_configured','Production configuration complete'),
 ('migrations_prepared','Database migrations addressed'),
 ('backups_ready','Backups addressed'),
 ('healthcheck_defined','Healthcheck addressed'),
 ('monitoring_ready','Monitoring addressed'),
 ('rollback_defined','Rollback plan defined'),
 ('deployment_confirmed','Deployment confirmed'),
 ('release_report_ready','Release Report ready')) c(code,label)
WHERE p.phase_type='DEPLOY' ON CONFLICT(phase_id,code) DO NOTHING;
