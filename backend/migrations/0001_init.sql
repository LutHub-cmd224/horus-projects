CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TYPE workspace_role AS ENUM ('OWNER','ADMIN','MEMBER','VIEWER');
CREATE TYPE project_status AS ENUM ('DRAFT','ACTIVE','PAUSED','COMPLETED','ARCHIVED');
CREATE TYPE phase_type AS ENUM ('ANALYZE','MODEL','DESIGN','BUILD','TEST','DEPLOY');
CREATE TYPE phase_status AS ENUM ('LOCKED','AVAILABLE','IN_PROGRESS','VALIDATED');
CREATE TYPE deliverable_status AS ENUM ('DRAFT','READY','VALIDATED');
CREATE TYPE requirement_type AS ENUM ('FUNCTIONAL','TECHNICAL','SECURITY','PERFORMANCE');
CREATE TYPE priority_level AS ENUM ('LOW','MEDIUM','HIGH','CRITICAL');
CREATE TYPE requirement_status AS ENUM ('DRAFT','APPROVED','IN_PROGRESS','DONE','REJECTED');
CREATE TYPE task_status AS ENUM ('TODO','IN_PROGRESS','BLOCKED','DONE','CANCELLED');
CREATE TYPE decision_status AS ENUM ('PROPOSED','ACCEPTED','SUPERSEDED','REJECTED');

CREATE TABLE users (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(), email VARCHAR(255) NOT NULL UNIQUE,
 password_hash TEXT NOT NULL, display_name VARCHAR(120), created_at TIMESTAMPTZ NOT NULL DEFAULT now(), updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE workspaces (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(), name VARCHAR(120) NOT NULL, slug VARCHAR(120) NOT NULL UNIQUE,
 created_by UUID NOT NULL REFERENCES users(id), created_at TIMESTAMPTZ NOT NULL DEFAULT now(), updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE workspace_members (
 workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE, user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
 role workspace_role NOT NULL DEFAULT 'MEMBER', joined_at TIMESTAMPTZ NOT NULL DEFAULT now(), PRIMARY KEY(workspace_id,user_id)
);
CREATE TABLE projects (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(), workspace_id UUID NOT NULL REFERENCES workspaces(id), name VARCHAR(160) NOT NULL,
 slug VARCHAR(160) NOT NULL, description TEXT, status project_status NOT NULL DEFAULT 'DRAFT', created_by UUID NOT NULL REFERENCES users(id),
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(), updated_at TIMESTAMPTZ NOT NULL DEFAULT now(), deleted_at TIMESTAMPTZ
);
CREATE UNIQUE INDEX projects_workspace_slug_active_uq ON projects(workspace_id,slug) WHERE deleted_at IS NULL;
CREATE TABLE phases (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(), project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE, phase_type phase_type NOT NULL,
 position SMALLINT NOT NULL CHECK(position BETWEEN 1 AND 6), status phase_status NOT NULL DEFAULT 'LOCKED', started_at TIMESTAMPTZ,
 validated_at TIMESTAMPTZ, created_at TIMESTAMPTZ NOT NULL DEFAULT now(), updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 UNIQUE(project_id,phase_type), UNIQUE(project_id,position)
);
CREATE TABLE deliverables (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(), phase_id UUID NOT NULL REFERENCES phases(id) ON DELETE CASCADE, title VARCHAR(180) NOT NULL,
 type VARCHAR(80) NOT NULL, content JSONB NOT NULL DEFAULT '{}'::jsonb, version INT NOT NULL DEFAULT 1 CHECK(version > 0),
 status deliverable_status NOT NULL DEFAULT 'DRAFT', created_by UUID NOT NULL REFERENCES users(id), created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT now(), deleted_at TIMESTAMPTZ
);
CREATE TABLE requirements (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(), project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE, code VARCHAR(30) NOT NULL,
 title VARCHAR(180) NOT NULL, description TEXT, type requirement_type NOT NULL, priority priority_level NOT NULL DEFAULT 'MEDIUM',
 status requirement_status NOT NULL DEFAULT 'DRAFT', created_by UUID NOT NULL REFERENCES users(id), created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT now(), deleted_at TIMESTAMPTZ
);
CREATE UNIQUE INDEX requirements_project_code_active_uq ON requirements(project_id,code) WHERE deleted_at IS NULL;
CREATE TABLE tasks (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(), project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
 requirement_id UUID REFERENCES requirements(id), phase_id UUID REFERENCES phases(id), code VARCHAR(30) NOT NULL, title VARCHAR(180) NOT NULL,
 description TEXT, status task_status NOT NULL DEFAULT 'TODO', priority priority_level NOT NULL DEFAULT 'MEDIUM', assigned_to UUID REFERENCES users(id),
 created_by UUID NOT NULL REFERENCES users(id), created_at TIMESTAMPTZ NOT NULL DEFAULT now(), updated_at TIMESTAMPTZ NOT NULL DEFAULT now(), deleted_at TIMESTAMPTZ
);
CREATE UNIQUE INDEX tasks_project_code_active_uq ON tasks(project_id,code) WHERE deleted_at IS NULL;
CREATE TABLE decisions (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(), project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE, phase_id UUID REFERENCES phases(id),
 code VARCHAR(30) NOT NULL, title VARCHAR(180) NOT NULL, context TEXT, decision TEXT NOT NULL, alternatives JSONB NOT NULL DEFAULT '[]'::jsonb,
 consequences TEXT, status decision_status NOT NULL DEFAULT 'PROPOSED', decided_by UUID NOT NULL REFERENCES users(id), decided_at TIMESTAMPTZ,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(), updated_at TIMESTAMPTZ NOT NULL DEFAULT now(), deleted_at TIMESTAMPTZ
);
CREATE UNIQUE INDEX decisions_project_code_active_uq ON decisions(project_id,code) WHERE deleted_at IS NULL;
CREATE TABLE validation_criteria (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(), phase_id UUID NOT NULL REFERENCES phases(id) ON DELETE CASCADE, code VARCHAR(50) NOT NULL,
 label VARCHAR(180) NOT NULL, required BOOLEAN NOT NULL DEFAULT true, completed BOOLEAN NOT NULL DEFAULT false, completed_by UUID REFERENCES users(id),
 completed_at TIMESTAMPTZ, UNIQUE(phase_id,code)
);
CREATE TABLE phase_validations (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(), phase_id UUID NOT NULL REFERENCES phases(id) ON DELETE CASCADE,
 validated_by UUID NOT NULL REFERENCES users(id), comment TEXT, created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX workspace_members_user_idx ON workspace_members(user_id);
CREATE INDEX projects_workspace_idx ON projects(workspace_id) WHERE deleted_at IS NULL;
CREATE INDEX phases_project_idx ON phases(project_id);
CREATE INDEX deliverables_phase_idx ON deliverables(phase_id) WHERE deleted_at IS NULL;
CREATE INDEX requirements_project_idx ON requirements(project_id) WHERE deleted_at IS NULL;
CREATE INDEX tasks_project_idx ON tasks(project_id) WHERE deleted_at IS NULL;
CREATE INDEX tasks_requirement_idx ON tasks(requirement_id) WHERE deleted_at IS NULL;
CREATE INDEX decisions_project_idx ON decisions(project_id) WHERE deleted_at IS NULL;
