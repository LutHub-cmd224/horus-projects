CREATE TABLE test_cases (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
 phase_id UUID NOT NULL REFERENCES phases(id) ON DELETE CASCADE,
 requirement_id UUID REFERENCES requirements(id),
 task_id UUID REFERENCES tasks(id),
 code VARCHAR(30) NOT NULL,
 title VARCHAR(180) NOT NULL,
 test_type VARCHAR(20) NOT NULL CHECK(test_type IN ('UNIT','INTEGRATION','E2E','SECURITY','PERFORMANCE','MANUAL')),
 expected_result TEXT NOT NULL,
 actual_result TEXT,
 status VARCHAR(20) NOT NULL DEFAULT 'NOT_RUN' CHECK(status IN ('NOT_RUN','PASSED','FAILED','BLOCKED')),
 executed_by UUID REFERENCES users(id),
 executed_at TIMESTAMPTZ,
 created_by UUID NOT NULL REFERENCES users(id),
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 deleted_at TIMESTAMPTZ,
 UNIQUE(phase_id,code)
);

CREATE TABLE test_defects (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
 phase_id UUID NOT NULL REFERENCES phases(id) ON DELETE CASCADE,
 test_case_id UUID NOT NULL REFERENCES test_cases(id),
 title VARCHAR(180) NOT NULL,
 description TEXT NOT NULL,
 severity VARCHAR(20) NOT NULL CHECK(severity IN ('BLOCKING','NON_BLOCKING')),
 status VARCHAR(20) NOT NULL DEFAULT 'OPEN' CHECK(status IN ('OPEN','RESOLVED','ACCEPTED')),
 resolution TEXT,
 created_by UUID NOT NULL REFERENCES users(id),
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 deleted_at TIMESTAMPTZ
);

CREATE INDEX test_cases_phase_idx ON test_cases(phase_id) WHERE deleted_at IS NULL;
CREATE INDEX test_defects_phase_idx ON test_defects(phase_id) WHERE deleted_at IS NULL;

INSERT INTO validation_criteria(phase_id,code,label,required)
SELECT p.id,c.code,c.label,true FROM phases p CROSS JOIN (VALUES
 ('build_plan_available','Validated Build Plan available'),
 ('test_cases_defined','Test cases defined'),
 ('test_traceability_complete','Test traceability complete'),
 ('tests_executed','All tests executed'),
 ('tests_passing','All tests passing'),
 ('no_blocking_defects','No unresolved blocking defects'),
 ('test_report_ready','Test report ready')) c(code,label)
WHERE p.phase_type='TEST' ON CONFLICT(phase_id,code) DO NOTHING;
