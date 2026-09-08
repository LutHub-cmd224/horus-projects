use std::collections::HashMap;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    auth::{TokenType, decode_token},
    state::AppState,
};

#[derive(Debug, Deserialize, Serialize)]
pub struct AttributeInput {
    pub conceptual_name: String,
    pub logical_name: Option<String>,
    pub physical_name: Option<String>,
    pub data_type: Option<String>,
    pub is_primary_key: bool,
    pub is_unique: bool,
    pub is_nullable: bool,
    pub default_value: Option<String>,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct EntityInput {
    pub conceptual_name: String,
    pub logical_name: Option<String>,
    pub physical_name: Option<String>,
    pub description: Option<String>,
    pub attributes: Vec<AttributeInput>,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct RelationshipInput {
    pub name: String,
    pub source_entity: String,
    pub target_entity: String,
    pub source_cardinality: String,
    pub target_cardinality: String,
    pub description: Option<String>,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct BusinessRuleInput {
    pub title: String,
    pub description: String,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct ModelWorkspace {
    pub entities: Vec<EntityInput>,
    pub relationships: Vec<RelationshipInput>,
    pub business_rules: Vec<BusinessRuleInput>,
    pub artifacts: Vec<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/phases/{phase_id}/model", get(load_model).put(save_model))
        .route(
            "/phases/{phase_id}/model/artifacts/{level}",
            post(generate_artifact),
        )
}

fn user(headers: &HeaderMap, state: &AppState) -> Result<Uuid, StatusCode> {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let claims = decode_token(token, &state.jwt_secret).map_err(|_| StatusCode::UNAUTHORIZED)?;
    if claims.token_type != TokenType::Access {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(claims.sub)
}

async fn access(
    state: &AppState,
    phase_id: Uuid,
    user_id: Uuid,
) -> Result<(String, String), StatusCode> {
    sqlx::query_as::<_, (String, String)>("SELECT wm.role::text, ph.status::text FROM phases ph JOIN projects p ON p.id=ph.project_id JOIN workspace_members wm ON wm.workspace_id=p.workspace_id WHERE ph.id=$1 AND ph.phase_type='MODEL' AND wm.user_id=$2 AND p.deleted_at IS NULL")
        .bind(phase_id).bind(user_id).fetch_optional(&state.db).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::FORBIDDEN)
}

fn writable(role: &str, status: &str) -> Result<(), StatusCode> {
    if role == "VIEWER" {
        return Err(StatusCode::FORBIDDEN);
    }
    if status == "LOCKED" || status == "VALIDATED" {
        return Err(StatusCode::CONFLICT);
    }
    Ok(())
}

async fn workspace(state: &AppState, phase_id: Uuid) -> Result<ModelWorkspace, StatusCode> {
    let entity_rows = sqlx::query_as::<_, (Uuid,String,Option<String>,Option<String>,Option<String>)>("SELECT id,conceptual_name,logical_name,physical_name,description FROM model_entities WHERE phase_id=$1 ORDER BY position,created_at").bind(phase_id).fetch_all(&state.db).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut entities = Vec::new();
    for (id, conceptual_name, logical_name, physical_name, description) in entity_rows {
        let attrs = sqlx::query_as::<_, (String,Option<String>,Option<String>,Option<String>,bool,bool,bool,Option<String>)>("SELECT conceptual_name,logical_name,physical_name,data_type,is_primary_key,is_unique,is_nullable,default_value FROM model_attributes WHERE entity_id=$1 ORDER BY position,created_at").bind(id).fetch_all(&state.db).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        entities.push(EntityInput {
            conceptual_name,
            logical_name,
            physical_name,
            description,
            attributes: attrs
                .into_iter()
                .map(|a| AttributeInput {
                    conceptual_name: a.0,
                    logical_name: a.1,
                    physical_name: a.2,
                    data_type: a.3,
                    is_primary_key: a.4,
                    is_unique: a.5,
                    is_nullable: a.6,
                    default_value: a.7,
                })
                .collect(),
        });
    }
    let relationships = sqlx::query_as::<_, (String,String,String,String,String,Option<String>)>("SELECT r.name,s.conceptual_name,t.conceptual_name,r.source_cardinality,r.target_cardinality,r.description FROM model_relationships r JOIN model_entities s ON s.id=r.source_entity_id JOIN model_entities t ON t.id=r.target_entity_id WHERE r.phase_id=$1 ORDER BY r.created_at").bind(phase_id).fetch_all(&state.db).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.into_iter().map(|r| RelationshipInput{name:r.0,source_entity:r.1,target_entity:r.2,source_cardinality:r.3,target_cardinality:r.4,description:r.5}).collect();
    let business_rules = sqlx::query_as::<_, (String, String)>(
        "SELECT title,description FROM business_rules WHERE phase_id=$1 ORDER BY code",
    )
    .bind(phase_id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .into_iter()
    .map(|r| BusinessRuleInput {
        title: r.0,
        description: r.1,
    })
    .collect();
    let artifacts = sqlx::query_scalar::<_, String>("SELECT replace(type,'MODEL_','') FROM deliverables WHERE phase_id=$1 AND type LIKE 'MODEL_%' AND deleted_at IS NULL ORDER BY type").bind(phase_id).fetch_all(&state.db).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(ModelWorkspace {
        entities,
        relationships,
        business_rules,
        artifacts,
    })
}

async fn load_model(
    State(state): State<AppState>,
    Path(phase_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<ModelWorkspace>, StatusCode> {
    let u = user(&headers, &state)?;
    access(&state, phase_id, u).await?;
    Ok(Json(workspace(&state, phase_id).await?))
}

async fn save_model(
    State(state): State<AppState>,
    Path(phase_id): Path<Uuid>,
    headers: HeaderMap,
    Json(payload): Json<ModelWorkspace>,
) -> Result<Json<ModelWorkspace>, StatusCode> {
    let u = user(&headers, &state)?;
    let (role, status) = access(&state, phase_id, u).await?;
    writable(&role, &status)?;
    if payload.entities.iter().any(|e| {
        e.conceptual_name.trim().is_empty()
            || e.attributes
                .iter()
                .any(|a| a.conceptual_name.trim().is_empty())
    }) || payload
        .business_rules
        .iter()
        .any(|r| r.title.trim().is_empty() || r.description.trim().is_empty())
    {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let cards = ["0..1", "1", "0..N", "1..N"];
    if payload.relationships.iter().any(|r| {
        r.name.trim().is_empty()
            || !cards.contains(&r.source_cardinality.as_str())
            || !cards.contains(&r.target_cardinality.as_str())
    }) {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    sqlx::query("DELETE FROM model_entities WHERE phase_id=$1")
        .bind(phase_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    sqlx::query("DELETE FROM business_rules WHERE phase_id=$1")
        .bind(phase_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut ids = HashMap::new();
    for (position, e) in payload.entities.iter().enumerate() {
        let id=sqlx::query_scalar::<_,Uuid>("INSERT INTO model_entities(phase_id,conceptual_name,logical_name,physical_name,description,position) VALUES($1,$2,$3,$4,$5,$6) RETURNING id").bind(phase_id).bind(e.conceptual_name.trim()).bind(&e.logical_name).bind(&e.physical_name).bind(&e.description).bind(position as i32).fetch_one(&mut *tx).await.map_err(|_|StatusCode::CONFLICT)?;
        ids.insert(e.conceptual_name.clone(), id);
        for (p, a) in e.attributes.iter().enumerate() {
            sqlx::query("INSERT INTO model_attributes(entity_id,conceptual_name,logical_name,physical_name,data_type,is_primary_key,is_unique,is_nullable,default_value,position) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)").bind(id).bind(a.conceptual_name.trim()).bind(&a.logical_name).bind(&a.physical_name).bind(&a.data_type).bind(a.is_primary_key).bind(a.is_unique).bind(a.is_nullable).bind(&a.default_value).bind(p as i32).execute(&mut *tx).await.map_err(|_|StatusCode::CONFLICT)?;
        }
    }
    for r in &payload.relationships {
        let source = *ids
            .get(&r.source_entity)
            .ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
        let target = *ids
            .get(&r.target_entity)
            .ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
        sqlx::query("INSERT INTO model_relationships(phase_id,name,source_entity_id,target_entity_id,source_cardinality,target_cardinality,description) VALUES($1,$2,$3,$4,$5,$6,$7)").bind(phase_id).bind(r.name.trim()).bind(source).bind(target).bind(&r.source_cardinality).bind(&r.target_cardinality).bind(&r.description).execute(&mut *tx).await.map_err(|_|StatusCode::CONFLICT)?;
    }
    for (i, r) in payload.business_rules.iter().enumerate() {
        sqlx::query(
            "INSERT INTO business_rules(phase_id,code,title,description) VALUES($1,$2,$3,$4)",
        )
        .bind(phase_id)
        .bind(format!("BR-{:03}", i + 1))
        .bind(r.title.trim())
        .bind(r.description.trim())
        .execute(&mut *tx)
        .await
        .map_err(|_| StatusCode::CONFLICT)?;
    }
    sqlx::query("UPDATE phases SET status='IN_PROGRESS',started_at=COALESCE(started_at,now()),updated_at=now() WHERE id=$1 AND status='AVAILABLE'").bind(phase_id).execute(&mut *tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    for (code, done) in [
        ("entities_defined", !payload.entities.is_empty()),
        ("relations_defined", !payload.relationships.is_empty()),
        ("business_rules_defined", !payload.business_rules.is_empty()),
    ] {
        sqlx::query("UPDATE validation_criteria SET completed=$2,completed_by=CASE WHEN $2 THEN $3 ELSE NULL END,completed_at=CASE WHEN $2 THEN now() ELSE NULL END WHERE phase_id=$1 AND code=$4").bind(phase_id).bind(done).bind(u).bind(code).execute(&mut *tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(workspace(&state, phase_id).await?))
}

async fn generate_artifact(
    State(state): State<AppState>,
    Path((phase_id, level)): Path<(Uuid, String)>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    let u = user(&headers, &state)?;
    let (role, status) = access(&state, phase_id, u).await?;
    writable(&role, &status)?;
    let level = level.to_uppercase();
    if !matches!(level.as_str(), "MCD" | "MLD" | "MPD") {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let model = workspace(&state, phase_id).await?;
    let ready = match level.as_str() {
        "MCD" => !model.entities.is_empty() && !model.relationships.is_empty(),
        "MLD" => {
            !model.entities.is_empty()
                && model.entities.iter().all(|e| {
                    e.logical_name
                        .as_deref()
                        .is_some_and(|v| !v.trim().is_empty())
                        && !e.attributes.is_empty()
                        && e.attributes.iter().all(|a| {
                            a.logical_name
                                .as_deref()
                                .is_some_and(|v| !v.trim().is_empty())
                        })
                })
        }
        _ => {
            !model.entities.is_empty()
                && model.entities.iter().all(|e| {
                    e.physical_name
                        .as_deref()
                        .is_some_and(|v| !v.trim().is_empty())
                        && e.attributes.iter().any(|a| a.is_primary_key)
                        && e.attributes.iter().all(|a| {
                            a.physical_name
                                .as_deref()
                                .is_some_and(|v| !v.trim().is_empty())
                                && a.data_type.as_deref().is_some_and(|v| !v.trim().is_empty())
                        })
                })
        }
    };
    if !ready {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let content = json!({"level":level,"model":model});
    let kind = format!("MODEL_{level}");
    let title = format!("{level} — Model workspace");
    let updated=sqlx::query("UPDATE deliverables SET content=$3,status='READY',version=version+1,updated_at=now() WHERE phase_id=$1 AND type=$2 AND deleted_at IS NULL").bind(phase_id).bind(&kind).bind(&content).execute(&state.db).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    if updated.rows_affected() == 0 {
        sqlx::query("INSERT INTO deliverables(phase_id,title,type,content,status,created_by) VALUES($1,$2,$3,$4,'READY',$5)").bind(phase_id).bind(title).bind(&kind).bind(&content).bind(u).execute(&state.db).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    sqlx::query("UPDATE validation_criteria SET completed=true,completed_by=$2,completed_at=now() WHERE phase_id=$1 AND code=$3").bind(phase_id).bind(u).bind(format!("{}_defined",level.to_lowercase())).execute(&state.db).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(content))
}
