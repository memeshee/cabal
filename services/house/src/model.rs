//! Wire types shared by the runner and the HTTP layer.

use serde::{Deserialize, Serialize};

use crate::dossier::SeatV;
use crate::llm::Via;

/// The contract's `RoundView`, as JSON.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoundV {
    pub id: u32,
    pub ticker: String,
    pub commitment: String,
    pub seats: Vec<SeatV>,
    pub funders: Vec<String>,
    pub opened_ms: u64,
    pub closes_ms: u64,
    pub reveal_by_ms: u64,
    pub supply: u64,
    pub reserve: String,
    pub bond: String,
    pub price: String,
    pub status: String,
    pub cabal_seat: Option<u8>,
    pub sides: Vec<String>,
    pub salt: Option<String>,
    pub accusers: Vec<AccuserV>,
    pub feed_len: u32,
    pub pruned: u32,
    pub payout: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccuserV {
    pub who: String,
    pub seat: u8,
    pub stake: String,
    /// Milliseconds after the table opened that the accusation was filed.
    #[serde(default)]
    pub at_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_accuser_keeps_the_time_the_contract_recorded() {
        // Regression: the field was dropped on the way through, so every accusation looked instantaneous.
        let a: AccuserV = serde_json::from_str(r#"{"who":"a.testnet","seat":1,"stake":"50000000000000000000000","at_ms":63977}"#).unwrap();
        assert_eq!(a.at_ms, 63977);
        let out = serde_json::to_value(&a).unwrap();
        assert_eq!(out["at_ms"], 63977);
        // an accuser from before the field existed still parses
        let old: AccuserV = serde_json::from_str(r#"{"who":"a.testnet","seat":1,"stake":"1"}"#).unwrap();
        assert_eq!(old.at_ms, 0);
    }
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Dealing,
    Playing,
    Revealing,
    Revealed,
    Voided,
    Failed,
}

impl Phase {
    pub fn is_final(self) -> bool {
        matches!(self, Phase::Revealed | Phase::Voided | Phase::Failed)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ChatLine {
    pub id: u32,
    pub ts_ms: u64,
    /// "@kaito", "you", or "house".
    pub who: String,
    pub text: String,
    /// "kol" | "you" | "house"
    pub kind: &'static str,
    pub via: Option<Via>,
    /// Set when this line is a caller's answer to one of the standard questions.
    pub claim_q: Option<u8>,
    pub claim_seat: Option<u8>,
    pub claim_tag: Option<&'static str>,
    /// Whether the claim was true. Withheld (None) until the table is settled.
    pub claim_true: Option<bool>,
}

#[derive(Serialize)]
pub struct ActionResult {
    pub ok: bool,
    pub hash: Option<String>,
    pub explorer_url: Option<String>,
    pub error: Option<String>,
    pub detail: Option<serde_json::Value>,
}

impl ActionResult {
    pub fn fail(msg: impl Into<String>) -> Self {
        Self { ok: false, hash: None, explorer_url: None, error: Some(msg.into()), detail: None }
    }
}
