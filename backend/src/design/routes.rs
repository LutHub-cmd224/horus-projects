use crate::{
    auth::{TokenType, decode_token},
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct Component {
    pub name: String,
    pub category: String,
    pub responsibility: String,
    pub technology: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Connection {
    pub source_name: String,
    pub target_name: String,
    pub protocol: String,
    pub description: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct DesignWorkspace {
    pub components: Vec<Component>,
    pub connections: Vec<Connection>,
    pub ux_flows: Value,
    pub features: Value,
    pub api_contracts: Value,
    pub security_controls: Value,
    pub decisions: Value,
    pub artifacts: Vec<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/phases/{phase_id}/design", get(load).put(save))
        .route("/phases/{phase_id}/design/artifacts/{kind}", post(artifact))
}
fn user(h: &HeaderMap, s: &AppState) -> Result<Uuid, StatusCode> {
    let t = h
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let c = decode_token(t, &s.jwt_secret).map_err(|_| StatusCode::UNAUTHORIZED)?;
    if c.token_type != TokenType::Access {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(c.sub)
}
async fn access(s: &AppState, p: Uuid, u: Uuid) -> Result<(String, String), StatusCode> {
    sqlx::query_as("SELECT wm.role::text,ph.status::text FROM phases ph JOIN projects pr ON pr.id=ph.project_id JOIN workspace_members wm ON wm.workspace_id=pr.workspace_id WHERE ph.id=$1 AND ph.phase_type='DESIGN' AND wm.user_id=$2").bind(p).bind(u).fetch_optional(&s.db).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::FORBIDDEN)
}
fn array(v: &Value) -> bool {
    v.as_array().is_some_and(|a| !a.is_empty())
}

fn has_accepted_decision(v: &Value) -> bool {
    v.as_array().is_some_and(|decisions| {
        decisions
            .iter()
            .any(|decision| decision.get("status").and_then(Value::as_str) == Some("ACCEPTED"))
    })
}
async fn workspace(s: &AppState, p: Uuid) -> Result<DesignWorkspace, StatusCode> {
    let components=sqlx::query_as::<_,(String,String,String,String)>("SELECT name,category,responsibility,technology FROM design_components WHERE phase_id=$1 ORDER BY position").bind(p).fetch_all(&s.db).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?.into_iter().map(|x|Component{name:x.0,category:x.1,responsibility:x.2,technology:x.3}).collect();
    let connections=sqlx::query_as::<_,(String,String,String,String)>("SELECT source_name,target_name,protocol,description FROM design_connections WHERE phase_id=$1 ORDER BY created_at").bind(p).fetch_all(&s.db).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?.into_iter().map(|x|Connection{source_name:x.0,target_name:x.1,protocol:x.2,description:x.3}).collect();
    let mut docs = std::collections::HashMap::new();
    for (k, v) in sqlx::query_as::<_, (String, Value)>(
        "SELECT kind,content FROM design_documents WHERE phase_id=$1",
    )
    .bind(p)
    .fetch_all(&s.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        docs.insert(k, v);
    }
    let artifacts=sqlx::query_scalar::<_,String>("SELECT replace(type,'DESIGN_','') FROM deliverables WHERE phase_id=$1 AND type LIKE 'DESIGN_%' AND deleted_at IS NULL").bind(p).fetch_all(&s.db).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(DesignWorkspace {
        components,
        connections,
        ux_flows: docs.remove("UX").unwrap_or(json!([])),
        features: docs.remove("FEATURES").unwrap_or(json!([])),
        api_contracts: docs.remove("API_CONTRACTS").unwrap_or(json!([])),
        security_controls: docs.remove("SECURITY").unwrap_or(json!([])),
        decisions: docs.remove("DECISIONS").unwrap_or(json!([])),
        artifacts,
    })
}
async fn load(
    State(s): State<AppState>,
    Path(p): Path<Uuid>,
    h: HeaderMap,
) -> Result<Json<DesignWorkspace>, StatusCode> {
    access(&s, p, user(&h, &s)?).await?;
    Ok(Json(workspace(&s, p).await?))
}
async fn save(
    State(s): State<AppState>,
    Path(p): Path<Uuid>,
    h: HeaderMap,
    Json(w): Json<DesignWorkspace>,
) -> Result<Json<DesignWorkspace>, StatusCode> {
    let u = user(&h, &s)?;
    let (r, st) = access(&s, p, u).await?;
    if r == "VIEWER" {
        return Err(StatusCode::FORBIDDEN);
    }
    if st == "LOCKED" || st == "VALIDATED" {
        return Err(StatusCode::CONFLICT);
    }
    if w.components.iter().any(|c| {
        c.name.trim().is_empty()
            || c.responsibility.trim().is_empty()
            || c.technology.trim().is_empty()
            || !matches!(
                c.category.as_str(),
                "FRONTEND" | "BACKEND" | "DATABASE" | "EXTERNAL"
            )
    }) {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let names: std::collections::HashSet<_> =
        w.components.iter().map(|c| c.name.as_str()).collect();
    if w.connections.iter().any(|c| {
        !names.contains(c.source_name.as_str())
            || !names.contains(c.target_name.as_str())
            || c.protocol.trim().is_empty()
    }) {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let mut tx =
        s.db.begin()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    sqlx::query("DELETE FROM design_connections WHERE phase_id=$1")
        .bind(p)
        .execute(&mut *tx)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    sqlx::query("DELETE FROM design_components WHERE phase_id=$1")
        .bind(p)
        .execute(&mut *tx)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    for (i, c) in w.components.iter().enumerate() {
        sqlx::query("INSERT INTO design_components(phase_id,name,category,responsibility,technology,position)VALUES($1,$2,$3,$4,$5,$6)").bind(p).bind(c.name.trim()).bind(&c.category).bind(c.responsibility.trim()).bind(c.technology.trim()).bind(i as i32).execute(&mut*tx).await.map_err(|_|StatusCode::CONFLICT)?;
    }
    for c in &w.connections {
        sqlx::query("INSERT INTO design_connections(phase_id,source_name,target_name,protocol,description)VALUES($1,$2,$3,$4,$5)").bind(p).bind(&c.source_name).bind(&c.target_name).bind(&c.protocol).bind(&c.description).execute(&mut*tx).await.map_err(|_|StatusCode::CONFLICT)?;
    }
    for (k, v) in [
        ("UX", &w.ux_flows),
        ("FEATURES", &w.features),
        ("API_CONTRACTS", &w.api_contracts),
        ("SECURITY", &w.security_controls),
        ("DECISIONS", &w.decisions),
    ] {
        if !v.is_array() {
            return Err(StatusCode::UNPROCESSABLE_ENTITY);
        }
        sqlx::query("INSERT INTO design_documents(phase_id,kind,content)VALUES($1,$2,$3)ON CONFLICT(phase_id,kind)DO UPDATE SET content=$3,updated_at=now()").bind(p).bind(k).bind(v).execute(&mut*tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    sqlx::query("UPDATE phases SET status='IN_PROGRESS',started_at=COALESCE(started_at,now()) WHERE id=$1 AND status='AVAILABLE'").bind(p).execute(&mut*tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    for (code, done) in [
        ("architecture_defined", !w.components.is_empty()),
        ("ux_flows_defined", array(&w.ux_flows)),
        ("features_specified", array(&w.features)),
        ("api_contracts_defined", array(&w.api_contracts)),
        ("security_reviewed", array(&w.security_controls)),
        (
            "technology_decisions_accepted",
            has_accepted_decision(&w.decisions),
        ),
    ] {
        sqlx::query("UPDATE validation_criteria SET completed=$2,completed_by=CASE WHEN $2 THEN $3 ELSE NULL END,completed_at=CASE WHEN $2 THEN now() ELSE NULL END WHERE phase_id=$1 AND code=$4").bind(p).bind(done).bind(u).bind(code).execute(&mut*tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(workspace(&s, p).await?))
}
async fn artifact(
    State(s): State<AppState>,
    Path((p, k)): Path<(Uuid, String)>,
    h: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    let u = user(&h, &s)?;
    let (r, st) = access(&s, p, u).await?;
    if r == "VIEWER" {
        return Err(StatusCode::FORBIDDEN);
    }
    if st == "LOCKED" || st == "VALIDATED" {
        return Err(StatusCode::CONFLICT);
    }
    let k = k.to_uppercase();
    if !matches!(
        k.as_str(),
        "ARCHITECTURE"
            | "UX"
            | "FEATURES"
            | "API_CONTRACTS"
            | "SECURITY"
            | "ADR_INDEX"
            | "DESIGN_PACK"
    ) {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let w = workspace(&s, p).await?;
    let ready = match k.as_str() {
        "ARCHITECTURE" => !w.components.is_empty(),
        "UX" => array(&w.ux_flows),
        "FEATURES" => array(&w.features),
        "API_CONTRACTS" => array(&w.api_contracts),
        "SECURITY" => array(&w.security_controls),
        "ADR_INDEX" => has_accepted_decision(&w.decisions),
        _ => {
            !w.components.is_empty()
                && array(&w.ux_flows)
                && array(&w.features)
                && array(&w.api_contracts)
                && array(&w.security_controls)
                && has_accepted_decision(&w.decisions)
        }
    };
    if !ready {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let content = json!({"kind":k,"design":w});
    let typ = format!("DESIGN_{k}");
    let mut tx =
        s.db.begin()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let n=sqlx::query("UPDATE deliverables SET content=$3,status='READY',version=version+1,updated_at=now() WHERE phase_id=$1 AND type=$2 AND deleted_at IS NULL").bind(p).bind(&typ).bind(&content).execute(&mut*tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    if n.rows_affected() == 0 {
        sqlx::query("INSERT INTO deliverables(phase_id,title,type,content,status,created_by)VALUES($1,$2,$3,$4,'READY',$5)").bind(p).bind(format!("{k} — Design workspace")).bind(&typ).bind(&content).bind(u).execute(&mut*tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    if k == "DESIGN_PACK" {
        sqlx::query("UPDATE validation_criteria SET completed=true,completed_by=$2,completed_at=now() WHERE phase_id=$1 AND code='design_pack_ready'").bind(p).bind(u).execute(&mut*tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(content))
}
