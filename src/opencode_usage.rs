//! Completed opencode message totals, without prompts, responses or credentials.

use chrono::{Local, NaiveDate, TimeZone};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;

type Ledger = BTreeMap<String, BTreeMap<String, u64>>;

fn ledger_path(day: NaiveDate) -> PathBuf {
    crate::state::state_file_path().with_file_name(format!("opencode-usage-{day}.json"))
}

fn read_ledger(day: NaiveDate) -> anyhow::Result<Ledger> {
    match std::fs::read(ledger_path(day)) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Ledger::new()),
        Err(e) => Err(e.into()),
    }
}

pub fn today_tokens(day: NaiveDate) -> u64 {
    read_ledger(day)
        .unwrap_or_default()
        .values()
        .flat_map(|messages| messages.values())
        .fold(0u64, |total, count| total.saturating_add(*count))
}

fn token_total(tokens: &Value) -> Option<u64> {
    if let Some(total) = tokens.get("total").and_then(Value::as_u64) {
        return Some(total);
    }
    // opencode's output already includes reasoning; input excludes cached tokens.
    tokens["input"]
        .as_u64()?
        .checked_add(tokens["output"].as_u64()?)?
        .checked_add(tokens["cache"]["read"].as_u64()?)?
        .checked_add(tokens["cache"]["write"].as_u64()?)
}

/// Reconcile one completed message independently of its session's live status.
pub fn ingest(v: &Value) -> anyhow::Result<()> {
    if !crate::config::read_config().usage_enabled || v["role"] != "assistant" {
        return Ok(());
    }
    let Some(session) = v["session_id"].as_str().filter(|id| !id.is_empty()) else {
        return Ok(());
    };
    let Some(message) = v["message_id"].as_str().filter(|id| !id.is_empty()) else {
        return Ok(());
    };
    let Some(completed) = v["completed"].as_i64().filter(|ms| *ms > 0) else {
        return Ok(());
    };
    let Some(completed) = Local.timestamp_millis_opt(completed).single() else {
        return Ok(());
    };
    let Some(total) = token_total(&v["tokens"]) else {
        return Ok(());
    };
    let day = completed.date_naive();
    let Some(_lock) = crate::state::acquire_state_lock() else {
        return Ok(());
    };
    // A damaged ledger must never be replaced with an empty one.
    let mut ledger = read_ledger(day)?;
    let previous = ledger
        .entry(session.to_owned())
        .or_default()
        .entry(message.to_owned())
        .or_default();
    if total <= *previous {
        return Ok(());
    }
    *previous = total;
    let path = ledger_path(day);
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    std::fs::write(&tmp, serde_json::to_vec(&ledger)?)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn totals_prefer_provider_total_and_never_add_reasoning_twice() {
        let mut tokens = json!({"input": 100, "output": 30, "reasoning": 20,
            "cache": {"read": 50, "write": 10}});
        assert_eq!(token_total(&tokens), Some(190));
        tokens["total"] = json!(140);
        assert_eq!(token_total(&tokens), Some(140));
        tokens["total"] = json!(-1);
        assert_eq!(token_total(&tokens), Some(190));
        tokens["input"] = json!(1.5);
        assert_eq!(token_total(&tokens), None);
        tokens["input"] = json!(u64::MAX);
        assert_eq!(token_total(&tokens), None);
    }
}
