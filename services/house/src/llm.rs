//! OpenAI-compatible chat client with a provider chain, spend caps, and a guaranteed answer.
//!
//! Order: NEAR AI Cloud (if a key is set) -> a second OpenAI-compatible endpoint (if configured) -> scripted lines.
//! Failures, empty replies (reasoning models can burn the whole budget thinking) and unsafe replies all fall through.

use std::{
    sync::atomic::{AtomicU32, AtomicU64, Ordering},
    time::Duration,
};

use serde_json::{json, Value};

use crate::{config::Config, kols::{sanitize, Msg}};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum Via {
    /// NEAR AI Cloud (TEE-hosted inference).
    NearAi,
    /// Another OpenAI-compatible endpoint.
    Llm,
    /// Scripted line.
    Scripted,
}

pub struct Llm {
    http: reqwest::Client,
    near_ai: Option<(String, String)>,
    fallback: Option<(String, String, String)>,
    cap: u32,
    used_today: AtomicU32,
    day: AtomicU64,
    /// Consecutive failures per provider; a provider that keeps failing is skipped for a while.
    near_fail: AtomicU32,
    fb_fail: AtomicU32,
}

fn today() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() / 86_400).unwrap_or(0)
}

impl Llm {
    pub fn new(cfg: &Config) -> Self {
        Self {
            http: reqwest::Client::builder().timeout(Duration::from_secs(8)).build().expect("http client"),
            near_ai: cfg.near_ai_key.clone().map(|k| (k, cfg.near_ai_model.clone())),
            fallback: cfg.fallback_llm.as_ref().map(|e| (e.base_url.clone(), e.api_key.clone(), e.model.clone())),
            cap: cfg.llm_daily_cap,
            used_today: AtomicU32::new(0),
            day: AtomicU64::new(today()),
            near_fail: AtomicU32::new(0),
            fb_fail: AtomicU32::new(0),
        }
    }

    pub fn configured(&self) -> bool {
        self.near_ai.is_some() || self.fallback.is_some()
    }

    fn budget_ok(&self) -> bool {
        let d = today();
        if self.day.swap(d, Ordering::Relaxed) != d {
            self.used_today.store(0, Ordering::Relaxed);
        }
        self.used_today.fetch_add(1, Ordering::Relaxed) < self.cap
    }

    /// Returns a cleaned line and which provider produced it, or `None` when the caller should script one.
    pub async fn line(&self, msgs: &[Msg]) -> Option<(String, Via)> {
        if !self.budget_ok() {
            return None;
        }
        if let Some((key, model)) = &self.near_ai {
            if self.near_fail.load(Ordering::Relaxed) < 4 {
                match self.ask("https://cloud-api.near.ai/v1", key, model, msgs, true).await {
                    Some(s) => {
                        self.near_fail.store(0, Ordering::Relaxed);
                        return Some((s, Via::NearAi));
                    }
                    None => {
                        self.near_fail.fetch_add(1, Ordering::Relaxed);
                    }
                }
            } else if self.near_fail.fetch_add(1, Ordering::Relaxed) > 40 {
                // Give it another chance after a while.
                self.near_fail.store(0, Ordering::Relaxed);
            }
        }
        if let Some((base, key, model)) = &self.fallback {
            if self.fb_fail.load(Ordering::Relaxed) < 4 {
                match self.ask(base, key, model, msgs, false).await {
                    Some(s) => {
                        self.fb_fail.store(0, Ordering::Relaxed);
                        return Some((s, Via::Llm));
                    }
                    None => {
                        self.fb_fail.fetch_add(1, Ordering::Relaxed);
                    }
                }
            } else if self.fb_fail.fetch_add(1, Ordering::Relaxed) > 40 {
                self.fb_fail.store(0, Ordering::Relaxed);
            }
        }
        None
    }

    async fn ask(&self, base: &str, key: &str, model: &str, msgs: &[Msg], near_ai: bool) -> Option<String> {
        let mut body = json!({
            "model": model,
            "messages": msgs,
            "max_tokens": 120,
            "temperature": 0.9,
        });
        if near_ai {
            // Reasoning models otherwise spend the whole budget thinking and return empty content.
            body["chat_template_kwargs"] = json!({ "enable_thinking": false });
        }
        let res = self
            .http
            .post(format!("{}/chat/completions", base.trim_end_matches('/')))
            .bearer_auth(key)
            .json(&body)
            .send()
            .await
            .ok()?;
        if !res.status().is_success() {
            tracing::warn!(status = %res.status(), "llm request failed");
            return None;
        }
        let v: Value = res.json().await.ok()?;
        sanitize(v["choices"][0]["message"]["content"].as_str()?)
    }
}
