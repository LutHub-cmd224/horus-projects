CREATE OR REPLACE FUNCTION horus_prepare_build_backlog(p_project_id UUID)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    v_created_by UUID;
    v_build_phase UUID;
BEGIN
    SELECT created_by INTO v_created_by FROM projects WHERE id = p_project_id AND deleted_at IS NULL;
    SELECT id INTO v_build_phase FROM phases WHERE project_id = p_project_id AND phase_type = 'BUILD';

    IF v_created_by IS NULL OR v_build_phase IS NULL THEN
        RETURN;
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM requirements WHERE project_id = p_project_id AND deleted_at IS NULL
    ) THEN
        WITH candidates AS (
            SELECT
                CASE d.kind WHEN 'SECURITY' THEN 'SECURITY'::requirement_type ELSE 'FUNCTIONAL'::requirement_type END AS requirement_type,
                NULLIF(trim(item->>'title'), '') AS title,
                NULLIF(trim(item->>'description'), '') AS description,
                CASE d.kind WHEN 'UX' THEN 1 WHEN 'FEATURES' THEN 2 ELSE 3 END AS kind_order,
                ordinality::INT AS item_order
            FROM phases p
            JOIN design_documents d ON d.phase_id = p.id
            CROSS JOIN LATERAL jsonb_array_elements(d.content) WITH ORDINALITY AS x(item, ordinality)
            WHERE p.project_id = p_project_id
              AND p.phase_type = 'DESIGN'
              AND p.status = 'VALIDATED'
              AND d.kind IN ('UX','FEATURES','SECURITY')
        ), numbered AS (
            SELECT *, row_number() OVER (ORDER BY kind_order, item_order, title) AS rn
            FROM candidates
            WHERE title IS NOT NULL
        )
        INSERT INTO requirements(project_id, code, title, description, type, priority, status, created_by)
        SELECT
            p_project_id,
            'AUTO-' || lpad(rn::TEXT, 3, '0'),
            left(title, 180),
            COALESCE(description, 'Besoin proposé automatiquement par HORUS à partir de la conception validée.'),
            requirement_type,
            'MEDIUM'::priority_level,
            'APPROVED'::requirement_status,
            v_created_by
        FROM numbered
        ON CONFLICT DO NOTHING;
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM tasks WHERE phase_id = v_build_phase AND deleted_at IS NULL
    ) THEN
        WITH source AS (
            SELECT id, title, description, row_number() OVER (ORDER BY code) AS rn
            FROM requirements
            WHERE project_id = p_project_id AND deleted_at IS NULL AND status <> 'REJECTED'
        )
        INSERT INTO tasks(project_id, requirement_id, phase_id, code, title, description, status, priority, created_by)
        SELECT
            p_project_id,
            id,
            v_build_phase,
            'TASK-AUTO-' || lpad(rn::TEXT, 3, '0'),
            left(title, 180),
            COALESCE(description, 'Élément proposé automatiquement par HORUS.'),
            'TODO'::task_status,
            'MEDIUM'::priority_level,
            v_created_by
        FROM source
        ON CONFLICT DO NOTHING;
    END IF;

    UPDATE validation_criteria
    SET completed = true,
        completed_by = v_created_by,
        completed_at = COALESCE(completed_at, now())
    WHERE phase_id = v_build_phase
      AND code IN ('backlog_ready','requirements_linked')
      AND EXISTS (SELECT 1 FROM tasks WHERE phase_id = v_build_phase AND deleted_at IS NULL);
END;
$$;

CREATE OR REPLACE FUNCTION horus_prepare_build_backlog_after_design_validation()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF NEW.phase_type = 'DESIGN' AND NEW.status = 'VALIDATED' AND OLD.status IS DISTINCT FROM NEW.status THEN
        PERFORM horus_prepare_build_backlog(NEW.project_id);
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS trg_horus_prepare_build_backlog ON phases;
CREATE TRIGGER trg_horus_prepare_build_backlog
AFTER UPDATE OF status ON phases
FOR EACH ROW
EXECUTE FUNCTION horus_prepare_build_backlog_after_design_validation();

DO $$
DECLARE
    project_row RECORD;
BEGIN
    FOR project_row IN
        SELECT DISTINCT project_id
        FROM phases
        WHERE phase_type = 'DESIGN' AND status = 'VALIDATED'
    LOOP
        PERFORM horus_prepare_build_backlog(project_row.project_id);
    END LOOP;
END;
$$;
