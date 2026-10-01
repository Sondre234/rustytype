use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use rusttype_core::Submission;
use serde::Deserialize;
use serde_json::json;

use crate::{AppState, service};

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(home))
        .route("/api/config", get(config))
        .route("/api/auth/github", post(auth_github))
        .route("/api/scores", post(post_score))
        .route("/api/leaderboard", get(get_leaderboard))
        .layer(DefaultBodyLimit::max(512 * 1024))
        .with_state(state)
}

fn error(status: StatusCode, message: impl ToString) -> Response {
    (status, Json(json!({ "error": message.to_string() }))).into_response()
}

async fn config(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(json!({ "github_client_id": state.github.client_id }))
}

#[derive(Deserialize)]
struct AuthRequest {
    access_token: String,
}

/// Trade a GitHub device-flow access token for a rusttype session token.
async fn auth_github(State(state): State<Arc<AppState>>, Json(request): Json<AuthRequest>) -> Response {
    let Some(github_user) = state.github.user_for_access_token(&request.access_token).await else {
        return error(StatusCode::UNAUTHORIZED, "GitHub did not accept that token for this app");
    };
    let db = Arc::clone(&state.db);
    let created = tokio::task::spawn_blocking(move || {
        let user = db.upsert_user(github_user.id, &github_user.login)?;
        let token = db.create_session(user.id)?;
        Ok::<_, rusqlite::Error>((user, token))
    })
    .await;
    match created {
        Ok(Ok((user, token))) => Json(json!({ "token": token, "login": user.login })).into_response(),
        _ => error(StatusCode::INTERNAL_SERVER_ERROR, "could not create session"),
    }
}

async fn post_score(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(submission): Json<Submission>,
) -> Response {
    let Some(token) = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::to_owned)
    else {
        return error(StatusCode::UNAUTHORIZED, "missing bearer token");
    };
    let db = Arc::clone(&state.db);
    let user = tokio::task::spawn_blocking(move || db.user_for_token(&token)).await;
    let Ok(Ok(Some(user))) = user else {
        return error(StatusCode::UNAUTHORIZED, "invalid or expired session; run `rusttype login`");
    };
    match service::submit(&state.db, user.id, submission).await {
        Ok(outcome) => Json(outcome).into_response(),
        Err(reason) => error(StatusCode::UNPROCESSABLE_ENTITY, reason),
    }
}

#[derive(Deserialize)]
struct BoardQuery {
    limit: Option<u32>,
    /// Snippet title for a per-snippet board; omit for the global board.
    snippet: Option<String>,
}

async fn get_leaderboard(State(state): State<Arc<AppState>>, Query(query): Query<BoardQuery>) -> Response {
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    Json(service::leaderboard(&state.db, query.snippet, limit).await).into_response()
}

async fn home(State(state): State<Arc<AppState>>) -> Html<String> {
    let rows: String = service::leaderboard(&state.db, None, 20)
        .await
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{:.0}</td><td>{:.1}%</td><td>{}</td></tr>",
                index + 1,
                entry.login, // GitHub logins are [A-Za-z0-9-]; nothing to escape
                entry.wpm,
                entry.accuracy,
                entry.snippet // always one of our own snippet titles
            )
        })
        .collect();
    let host = &state.public_host;
    Html(format!(
        "<!doctype html><meta charset=utf-8><meta name=viewport content='width=device-width'>\
         <title>rusttype</title>\
         <style>body{{font:16px/1.5 monospace;max-width:40rem;margin:3rem auto;padding:0 1rem;\
         background:#111;color:#ddd}}td,th{{padding:.15rem 1rem .15rem 0;text-align:left}}\
         code{{background:#222;padding:.1rem .4rem}}</style>\
         <h1>rusttype</h1><p>Type real Rust. Play over SSH, scores tied to your GitHub account:</p>\
         <p><code>ssh &lt;your-github-login&gt;@{host}</code></p>\
         <p>or try it first: <code>ssh guest@{host}</code></p>\
         <h2>leaderboard</h2><table><tr><th>#<th>player<th>wpm<th>acc<th>snippet</tr>{rows}</table>"
    ))
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use rusttype_core::{SNIPPETS, synthetic_run};
    use tower::ServiceExt;

    use super::*;
    use crate::{db::Db, github::Github};

    async fn post_run(app: &Router, token: Option<&str>, gap_ms: u32) -> (StatusCode, serde_json::Value) {
        let run = synthetic_run(SNIPPETS[0].0, gap_ms).unwrap();
        let mut request = Request::post("/api/scores").header("content-type", "application/json");
        if let Some(token) = token {
            request = request.header("authorization", format!("Bearer {token}"));
        }
        let response = app
            .clone()
            .oneshot(request.body(Body::from(serde_json::to_vec(&run).unwrap())).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        (status, serde_json::from_slice(&body).unwrap())
    }

    #[tokio::test]
    async fn scores_need_a_session_and_a_believable_run() {
        let db = Db::open(":memory:").unwrap();
        let user = db.upsert_user(1, "ada").unwrap();
        let token = db.create_session(user.id).unwrap();
        let app = router(Arc::new(AppState {
            db: Arc::new(db),
            github: Github::new("id".into(), "secret".into()),
            public_host: "example.test".into(),
        }));

        assert_eq!(post_run(&app, None, 120).await.0, StatusCode::UNAUTHORIZED);
        assert_eq!(post_run(&app, Some("wrong"), 120).await.0, StatusCode::UNAUTHORIZED);
        assert_eq!(post_run(&app, Some(&token), 5).await.0, StatusCode::UNPROCESSABLE_ENTITY);

        let (status, body) = post_run(&app, Some(&token), 120).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["rank"], 1);
        assert_eq!(body["personal_best"], true);
    }
}
