CREATE TABLE analyze_profiles (
    phase_id UUID PRIMARY KEY REFERENCES phases(id) ON DELETE CASCADE,
    problem TEXT NOT NULL DEFAULT '',
    target_audiences JSONB NOT NULL DEFAULT '[]'::jsonb,
    target_details TEXT NOT NULL DEFAULT '',
    value_proposition TEXT NOT NULL DEFAULT '',
    success_objectives JSONB NOT NULL DEFAULT '[]'::jsonb,
    budget TEXT NOT NULL DEFAULT '',
    deadline TEXT NOT NULL DEFAULT '',
    platform TEXT NOT NULL DEFAULT '',
    special_constraints TEXT NOT NULL DEFAULT '',
    constraints_unknown BOOLEAN NOT NULL DEFAULT false,
    mvp_features JSONB NOT NULL DEFAULT '[]'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO analyze_profiles (phase_id)
SELECT id FROM phases WHERE phase_type = 'ANALYZE'
ON CONFLICT (phase_id) DO NOTHING;
