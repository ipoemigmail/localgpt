//! HTTP server for LocalGPT
//!
//! Note: The chat endpoint creates a new agent per request because the Agent
//! struct contains SQLite connections that cannot be shared across threads.
//! For persistent session state, use the CLI interface instead.

use anyhow::Result;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

use crate::agent::{Agent, AgentConfig};
use crate::config::Config;
use crate::memory::MemoryManager;

pub struct Server {
    config: Config,
}

struct AppState {
    config: Config,
}

impl Server {
    pub fn new(config: &Config) -> Result<Self> {
        Ok(Self {
            config: config.clone(),
        })
    }

    pub async fn run(&self) -> Result<()> {
        let state = Arc::new(AppState {
            config: self.config.clone(),
        });

        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        let app = Router::new()
            .route("/health", get(health_check))
            .route("/api/chat", post(chat))
            .route("/api/memory/search", get(memory_search))
            .route("/api/memory/stats", get(memory_stats))
            .route("/api/status", get(status))
            .layer(cors)
            .with_state(state);

        let addr: SocketAddr =
            format!("{}:{}", self.config.server.bind, self.config.server.port).parse()?;

        info!("Starting HTTP server on http://{}", addr);

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }
}

// Error response type
struct AppError(StatusCode, String);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (self.0, self.1).into_response()
    }
}

// Health check endpoint
async fn health_check() -> &'static str {
    "OK"
}

// Status endpoint
#[derive(Serialize)]
struct StatusResponse {
    version: String,
    model: String,
    memory_chunks: usize,
}

async fn status(State(state): State<Arc<AppState>>) -> Json<StatusResponse> {
    let memory = MemoryManager::new(&state.config.memory).ok();

    Json(StatusResponse {
        version: env!("CARGO_PKG_VERSION").to_string(),
        model: state.config.agent.default_model.clone(),
        memory_chunks: memory.and_then(|m| m.chunk_count().ok()).unwrap_or(0),
    })
}

// Chat endpoint
#[derive(Deserialize)]
struct ChatRequest {
    message: String,
    model: Option<String>,
}

#[derive(Serialize)]
struct ChatResponse {
    response: String,
    model: String,
}

async fn chat(State(state): State<Arc<AppState>>, Json(request): Json<ChatRequest>) -> Response {
    // Create a new agent for this request
    // Note: This means no session persistence across HTTP requests
    let result = tokio::task::spawn_blocking({
        let config = state.config.clone();
        let message = request.message.clone();
        let model = request.model.clone();
        move || {
            // Run in blocking context since Agent isn't Send+Sync
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let memory = MemoryManager::new(&config.memory)?;

                let agent_config = AgentConfig {
                    model: model.unwrap_or(config.agent.default_model.clone()),
                    context_window: config.agent.context_window,
                    reserve_tokens: config.agent.reserve_tokens,
                };

                let mut agent = Agent::new(agent_config, &config, memory).await?;
                agent.new_session().await?;

                let response = agent.chat(&message).await?;
                let model = agent.model().to_string();

                Ok::<_, anyhow::Error>(ChatResponse { response, model })
            })
        }
    })
    .await;

    match result {
        Ok(Ok(response)) => Json(response).into_response(),
        Ok(Err(e)) => AppError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        Err(e) => AppError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

// Memory search endpoint
#[derive(Deserialize)]
struct SearchQuery {
    q: String,
    limit: Option<usize>,
}

#[derive(Serialize)]
struct SearchResult {
    file: String,
    line_start: i32,
    line_end: i32,
    content: String,
    score: f64,
}

#[derive(Serialize)]
struct SearchResponse {
    results: Vec<SearchResult>,
    query: String,
}

async fn memory_search(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
) -> Response {
    match memory_search_inner(&state.config.memory, &query.q, query.limit) {
        Ok(response) => Json(response).into_response(),
        Err(e) => AppError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

fn memory_search_inner(
    config: &crate::config::MemoryConfig,
    query: &str,
    limit: Option<usize>,
) -> Result<SearchResponse, anyhow::Error> {
    let memory = MemoryManager::new(config)?;

    let limit = limit.unwrap_or(10);
    let results = memory.search(query, limit)?;

    let results: Vec<SearchResult> = results
        .into_iter()
        .map(|r| SearchResult {
            file: r.file,
            line_start: r.line_start,
            line_end: r.line_end,
            content: r.content,
            score: r.score,
        })
        .collect();

    Ok(SearchResponse {
        results,
        query: query.to_string(),
    })
}

// Memory stats endpoint
#[derive(Serialize)]
struct StatsResponse {
    workspace: String,
    total_files: usize,
    total_chunks: usize,
    index_size_kb: u64,
}

async fn memory_stats(State(state): State<Arc<AppState>>) -> Response {
    match memory_stats_inner(&state.config.memory) {
        Ok(response) => Json(response).into_response(),
        Err(e) => AppError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

fn memory_stats_inner(
    config: &crate::config::MemoryConfig,
) -> Result<StatsResponse, anyhow::Error> {
    let memory = MemoryManager::new(config)?;
    let stats = memory.stats()?;

    Ok(StatsResponse {
        workspace: stats.workspace,
        total_files: stats.total_files,
        total_chunks: stats.total_chunks,
        index_size_kb: stats.index_size_kb,
    })
}
