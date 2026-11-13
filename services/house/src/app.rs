//! HTTP surface. Everything the browser needs; nothing that can reveal the hidden role.

use std::{net::SocketAddr, sync::Arc};

use axum::{
    extract::{ConnectInfo, Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    chain::YOCTO_PER_MNEAR,
    game::{now_ms, Game},
    model::{ActionResult, Phase},
};

type Ctx = Arc<Game>;
type Err = (StatusCode, Json<Value>);

fn err(code: StatusCode, msg: impl Into<String>) -> Err {
    (code, Json(json!({ "ok": false, "error": msg.into() })))
}

fn client_ip(h: &HeaderMap, peer: SocketAddr) -> String {
    h.get("fly-client-ip")
        .or_else(|| h.get("x-forwarded-for"))
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| peer.ip().to_string())
}

pub fn router(g: Ctx) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/guest", post(guest))
        .route("/api/table", post(new_table))
        .route("/api/table/{key}", get(table))
        .route("/api/table/{key}/me", get(me))
        .route("/api/table/{key}/buy", post(buy))
        .route("/api/table/{key}/sell", post(sell))
        .route("/api/table/{key}/expose", post(expose))
        .route("/api/table/{key}/ask", post(ask))
        .route("/api/round/{id}", get(round))
        .route("/api/rounds", get(rounds))
        .with_state(g)
        .layer(tower_http::compression::CompressionLayer::new())
        .layer(tower_http::trace::TraceLayer::new_for_http())
}

async fn health(State(g): State<Ctx>) -> Json<Value> {
    let bal = g.chain.balance(&g.chain.house).await;
    Json(json!({
        "ok": true,
        "network": g.cfg.network,
        "contract": g.chain.contract,
        "house": g.chain.house,
        "house_near": bal as f64 / (YOCTO_PER_MNEAR as f64 * 1000.0),
        "free_tables": g.free_slots(),
        "slots": g.cfg.slots,
        "dialogue": if g.llm.configured() { "model" } else { "scripted" },
        "uptime_s": g.started.elapsed().as_secs(),
        "now_ms": now_ms(),
    }))
}

#[derive(Deserialize)]
struct GuestReq {
    token: Option<String>,
}

async fn guest(State(g): State<Ctx>, ConnectInfo(peer): ConnectInfo<SocketAddr>, h: HeaderMap, Json(r): Json<GuestReq>) -> Result<Json<Value>, Err> {
    let ip = client_ip(&h, peer);
    match g.guest(r.token.as_deref(), &ip).await {
        Ok((token, acct, bal)) => Ok(Json(json!({ "ok": true, "token": token, "account": acct, "balance_near": bal as f64 / (YOCTO_PER_MNEAR as f64 * 1000.0) }))),
        Err(e) => Err(err(StatusCode::TOO_MANY_REQUESTS, e.to_string())),
    }
}

#[derive(Deserialize)]
struct NewTable {
    #[serde(default)]
    demo: bool,
}

async fn new_table(State(g): State<Ctx>, ConnectInfo(peer): ConnectInfo<SocketAddr>, h: HeaderMap, Json(r): Json<NewTable>) -> Result<Json<Value>, Err> {
    let ip = client_ip(&h, peer);
    match g.start_table(r.demo, &ip) {
        Ok(t) => Ok(Json(json!({ "ok": true, "key": t.key }))),
        Err(e) => Err(err(StatusCode::SERVICE_UNAVAILABLE, e.to_string())),
    }
}

#[derive(Deserialize)]
struct Since {
    #[serde(default)]
    feed: usize,
    #[serde(default)]
    chat: usize,
}

async fn table(State(g): State<Ctx>, Path(key): Path<String>, Query(s): Query<Since>) -> Result<Json<Value>, Err> {
    let t = g.table(&key).ok_or_else(|| err(StatusCode::NOT_FOUND, "That table is gone. Tables live for a few minutes; finished ones stay readable from the chain."))?;
    Ok(Json(serde_json::to_value(g.table_view(&t, s.feed, s.chat)).unwrap_or_default()))
}

#[derive(Deserialize)]
struct Tok {
    token: String,
}

async fn me(State(g): State<Ctx>, Path(key): Path<String>, Query(q): Query<Tok>) -> Result<Json<Value>, Err> {
    let t = g.table(&key).ok_or_else(|| err(StatusCode::NOT_FOUND, "no such table"))?;
    let who = g.parse_token(&q.token).ok_or_else(|| err(StatusCode::UNAUTHORIZED, "bad token"))?;
    let bal = g.chain.balance(&who).await;
    let v = g.table_view(&t, usize::MAX, usize::MAX);
    let (chips, mine) = match v.round {
        Some(round) => {
            let c: u64 = g.chain.view("chips_of", json!({ "round": round, "account_id": who })).await.unwrap_or(0);
            let acc = v.round_view.as_ref().and_then(|r| r.accusers.iter().find(|a| a.who == who.as_str()).cloned());
            (c, acc)
        }
        None => (0, None),
    };
    let value: String = match v.round {
        Some(round) if chips > 0 => g.chain.view::<String>("quote_sell", json!({ "round": round, "chips": chips })).await.unwrap_or_default(),
        _ => "0".into(),
    };
    Ok(Json(json!({ "ok": true, "account": who, "balance_near": bal as f64 / (YOCTO_PER_MNEAR as f64 * 1000.0), "chips": chips, "chips_value": value, "accusation": mine })))
}

fn resolve(g: &Game, key: &str, token: &str) -> Result<(Arc<crate::game::Table>, near_api::types::AccountId), Err> {
    let t = g.table(key).ok_or_else(|| err(StatusCode::NOT_FOUND, "no such table"))?;
    let who = g.parse_token(token).ok_or_else(|| err(StatusCode::UNAUTHORIZED, "bad token"))?;
    Ok((t, who))
}

#[derive(Deserialize)]
struct BuyReq {
    token: String,
    mnear: u64,
}

async fn buy(State(g): State<Ctx>, Path(key): Path<String>, Json(r): Json<BuyReq>) -> Result<Json<ActionResult>, Err> {
    let (t, who) = resolve(&g, &key, &r.token)?;
    Ok(Json(g.player_buy(&t, &who, r.mnear).await))
}

#[derive(Deserialize)]
struct SellReq {
    token: String,
    pct: u8,
}

async fn sell(State(g): State<Ctx>, Path(key): Path<String>, Json(r): Json<SellReq>) -> Result<Json<ActionResult>, Err> {
    let (t, who) = resolve(&g, &key, &r.token)?;
    Ok(Json(g.player_sell(&t, &who, r.pct).await))
}

#[derive(Deserialize)]
struct ExposeReq {
    token: String,
    seat: u8,
    mnear: u64,
}

async fn expose(State(g): State<Ctx>, Path(key): Path<String>, Json(r): Json<ExposeReq>) -> Result<Json<ActionResult>, Err> {
    let (t, who) = resolve(&g, &key, &r.token)?;
    Ok(Json(g.player_expose(&t, &who, r.seat, r.mnear).await))
}

#[derive(Deserialize)]
struct AskReq {
    token: String,
    seat: usize,
    text: String,
}

async fn ask(State(g): State<Ctx>, Path(key): Path<String>, Json(r): Json<AskReq>) -> Result<Json<Value>, Err> {
    let (t, who) = resolve(&g, &key, &r.token)?;
    match g.player_ask(&t, &who, r.seat, &r.text).await {
        Ok(lines) => Ok(Json(json!({ "ok": true, "lines": lines }))),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e }))),
    }
}

async fn round(State(g): State<Ctx>, Path(id): Path<u32>) -> Result<Json<Value>, Err> {
    match g.round_view(id).await {
        Ok(v) => Ok(Json(serde_json::to_value(v).unwrap_or_default())),
        Err(e) => Err(err(StatusCode::NOT_FOUND, e.to_string())),
    }
}

async fn rounds(State(g): State<Ctx>) -> Result<Json<Value>, Err> {
    let latest: u32 = g.chain.view("latest_round", json!({})).await.map_err(|e| err(StatusCode::BAD_GATEWAY, e.to_string()))?;
    let mut out = vec![];
    for id in (1..=latest).rev().take(12) {
        if let Ok(Some(r)) = g.chain.view::<Option<crate::model::RoundV>>("get_round", json!({ "round": id })).await {
            out.push(json!({
                "id": r.id, "ticker": r.ticker, "status": r.status, "opened_ms": r.opened_ms, "accusers": r.accusers.len(),
                "cabal": r.cabal_seat.and_then(|c| r.seats.get(c as usize)).map(|s| s.handle.clone()),
            }));
        }
    }
    let live: Vec<Phase> = vec![];
    let _ = live;
    Ok(Json(json!({ "ok": true, "latest": latest, "rounds": out })))
}
