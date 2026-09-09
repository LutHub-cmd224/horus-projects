use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    routing::get,
};
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    auth::{TokenType, decode_token},
    state::AppState,
};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct OverviewProject {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct OverviewPhase {
    pub id: Uuid,
    pub phase_type: String,
    pub position: i16,
    pub status: String,
    pub required_criteria: i64,
    pub completed_required_criteria: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct LatestDecision {
    pub code: String,
    pub title: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct ProjectOverviewResponse {
    pub project: OverviewProject,
    pub phases: Vec<OverviewPhase>,
    pub requirement_count: i64,
    pub open_task_count: i64,
    pub decision_count: i64,
    pub latest_decision: Option<LatestDecision>,
    pub next_action: Option<NextAction>,
}

#[derive(Debug, Serialize)]
pub struct NextAction {
    pub phase: String,
    pub phase_id: Uuid,
    pub step: String,
    pub title: String,
    pub cta: String,
    pub reason: String,
    pub blocked: bool,
}

fn array_has_text(value: &Value) -> bool {
    value.as_array().is_some_and(|items| {
        items
            .iter()
            .any(|item| item.as_str().is_some_and(|text| !text.trim().is_empty()))
    })
}

fn action_copy(phase: &str, code: &str) -> (&'static str, &'static str) {
    match (phase, code) {
        ("ANALYZE", "problem_defined") => (
            "Clarifier le problème principal",
            "HORUS doit comprendre ce qui ne fonctionne pas aujourd’hui.",
        ),
        ("ANALYZE", "target_user_defined") => (
            "Choisir les personnes concernées",
            "Les propositions deviennent précises quand HORUS sait pour qui tu construis.",
        ),
        ("ANALYZE", "value_proposition_defined") => (
            "Choisir le résultat apporté",
            "HORUS relie le problème à un bénéfice concret.",
        ),
        ("ANALYZE", "objectives_defined") => (
            "Définir un signe de réussite",
            "Un résultat observable permettra de vérifier que l’idée fonctionne.",
        ),
        ("ANALYZE", "constraints_defined") => (
            "Préciser les limites importantes",
            "HORUS adaptera ses propositions au contexte.",
        ),
        ("ANALYZE", "mvp_defined") => (
            "Choisir l’essentiel de la première version",
            "HORUS doit distinguer l’indispensable du reste.",
        ),
        ("MODEL", "entities_defined") => (
            "Choisir les éléments importants",
            "HORUS va proposer ce que l’application doit connaître.",
        ),
        ("MODEL", "relations_defined") => (
            "Confirmer comment les éléments fonctionnent ensemble",
            "Ces liens permettent à HORUS de préparer la structure.",
        ),
        ("MODEL", "business_rules_defined") => (
            "Confirmer les règles importantes",
            "Les règles évitent les incohérences dans le produit.",
        ),
        ("DESIGN", _) => (
            "Choisir comment le produit sera utilisé",
            "HORUS transformera les usages en proposition de conception.",
        ),
        ("BUILD", _) => (
            "Confirmer ce qu’il faut construire",
            "HORUS prépare une checklist à partir des besoins validés.",
        ),
        ("TEST", _) => (
            "Vérifier un scénario utilisateur",
            "HORUS doit confirmer que le produit fonctionne comme prévu.",
        ),
        ("DEPLOY", _) => (
            "Préparer la mise en ligne",
            "HORUS vérifie les protections avant de livrer le produit.",
        ),
        _ => (
            "Continuer le projet",
            "HORUS te guide vers la prochaine décision utile.",
        ),
    }
}

pub fn router() -> Router<AppState> {
    Router::new().route("/projects/{project_id}/overview", get(project_overview))
}

fn authenticated_user(headers: &HeaderMap, state: &AppState) -> Result<Uuid, StatusCode> {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let claims = decode_token(token, &state.jwt_secret).map_err(|_| StatusCode::UNAUTHORIZED)?;
    if claims.token_type != TokenType::Access {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(claims.sub)
}

async fn project_overview(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<ProjectOverviewResponse>, StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;

    let project = sqlx::query_as::<_, OverviewProject>(
        "SELECT p.id, p.name, p.description, p.status::text AS status FROM projects p JOIN workspace_members wm ON wm.workspace_id = p.workspace_id WHERE p.id = $1 AND wm.user_id = $2 AND p.deleted_at IS NULL",
    )
    .bind(project_id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let mut phases = sqlx::query_as::<_, OverviewPhase>(
        "SELECT p.id, p.phase_type::text AS phase_type, p.position, p.status::text AS status, COUNT(vc.id) FILTER (WHERE vc.required = true) AS required_criteria, COUNT(vc.id) FILTER (WHERE vc.required = true AND vc.completed = true) AS completed_required_criteria FROM phases p LEFT JOIN validation_criteria vc ON vc.phase_id = p.id WHERE p.project_id = $1 GROUP BY p.id ORDER BY p.position ASC",
    )
    .bind(project_id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(analyze) = phases
        .iter_mut()
        .find(|phase| phase.phase_type == "ANALYZE")
    {
        let profile = sqlx::query_as::<_, (String, Value, String, String, Value, String, String, String, String, bool, Value)>("SELECT problem,target_audiences,target_details,value_proposition,success_objectives,budget,deadline,platform,special_constraints,constraints_unknown,mvp_features FROM analyze_profiles WHERE phase_id=$1")
            .bind(analyze.id).fetch_optional(&state.db).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        analyze.completed_required_criteria = profile.map_or(0, |p| {
            [
                !p.0.trim().is_empty(),
                array_has_text(&p.1) || !p.2.trim().is_empty(),
                !p.3.trim().is_empty(),
                array_has_text(&p.4),
                p.9 || [&p.5, &p.6, &p.7, &p.8]
                    .iter()
                    .any(|value| !value.trim().is_empty()),
                array_has_text(&p.10),
            ]
            .into_iter()
            .filter(|done| *done)
            .count() as i64
        });
    }

    let current = phases
        .iter()
        .find(|phase| phase.status == "IN_PROGRESS")
        .or_else(|| phases.iter().find(|phase| phase.status == "AVAILABLE"));
    let next_action = if let Some(phase) = current {
        let code = if phase.phase_type == "ANALYZE" {
            let profile = sqlx::query_as::<_, (String, Value, String, String, Value, String, String, String, String, bool, Value)>("SELECT problem,target_audiences,target_details,value_proposition,success_objectives,budget,deadline,platform,special_constraints,constraints_unknown,mvp_features FROM analyze_profiles WHERE phase_id=$1")
                .bind(phase.id).fetch_optional(&state.db).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            profile.map_or_else(
                || "problem_defined".to_string(),
                |p| {
                    [
                        ("problem_defined", !p.0.trim().is_empty()),
                        (
                            "target_user_defined",
                            array_has_text(&p.1) || !p.2.trim().is_empty(),
                        ),
                        ("value_proposition_defined", !p.3.trim().is_empty()),
                        ("objectives_defined", array_has_text(&p.4)),
                        (
                            "constraints_defined",
                            p.9 || [&p.5, &p.6, &p.7, &p.8]
                                .iter()
                                .any(|value| !value.trim().is_empty()),
                        ),
                        ("mvp_defined", array_has_text(&p.10)),
                    ]
                    .into_iter()
                    .find(|(_, done)| !done)
                    .map_or("confirm", |(code, _)| code)
                    .to_string()
                },
            )
        } else {
            sqlx::query_scalar::<_, String>("SELECT code FROM validation_criteria WHERE phase_id=$1 AND required AND NOT completed ORDER BY created_at LIMIT 1")
                .bind(phase.id).fetch_optional(&state.db).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.unwrap_or_else(|| "confirm".into())
        };
        let (title, reason) = action_copy(&phase.phase_type, &code);
        Some(NextAction {
            phase: phase.phase_type.clone(),
            phase_id: phase.id,
            step: code,
            title: title.into(),
            cta: "Continuer".into(),
            reason: reason.into(),
            blocked: false,
        })
    } else {
        None
    };

    let requirement_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM requirements WHERE project_id = $1 AND deleted_at IS NULL",
    )
    .bind(project_id)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let open_task_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM tasks WHERE project_id = $1 AND deleted_at IS NULL AND status NOT IN ('DONE', 'CANCELLED')",
    )
    .bind(project_id)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let decision_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM decisions WHERE project_id = $1 AND deleted_at IS NULL",
    )
    .bind(project_id)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let latest_decision = sqlx::query_as::<_, LatestDecision>(
        "SELECT code, title, status::text AS status FROM decisions WHERE project_id = $1 AND deleted_at IS NULL ORDER BY created_at DESC LIMIT 1",
    )
    .bind(project_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(ProjectOverviewResponse {
        project,
        phases,
        requirement_count,
        open_task_count,
        decision_count,
        latest_decision,
        next_action,
    }))
}
