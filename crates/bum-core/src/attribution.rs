//! Attribute skills to each turn: direct / continued / command (spec section 4.3).
use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::model::SkillKind;

const CHAIN_GAP_SECS: i64 = 30 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum How {
    Direct,
    Continued,
    Command,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AttributedSkill {
    pub skill: String,
    pub how: How,
}

fn ts(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s).ok().map(|t| t.with_timezone(&Utc))
}

/// `prompts`: (key, ts) of a session; `uses`: (prompt_key, skill, kind); `compactions`: ts.
pub fn attribute(
    prompts: &[(String, String)],
    uses: &[(String, String, SkillKind)],
    compactions: &[String],
) -> HashMap<String, Vec<AttributedSkill>> {
    let mut sorted: Vec<_> = prompts.iter().filter_map(|(k, t)| ts(t).map(|t| (k, t))).collect();
    sorted.sort_by_key(|(_, t)| *t);
    let compactions: Vec<_> = compactions.iter().filter_map(|c| ts(c)).collect();

    let mut out = HashMap::new();
    let mut chain: Option<String> = None;
    let mut last: Option<DateTime<Utc>> = None;
    for (key, at) in sorted {
        if let Some(prev) = last {
            let compacted = compactions.iter().any(|c| *c > prev && *c <= at);
            if compacted || (at - prev).num_seconds() > CHAIN_GAP_SECS {
                chain = None;
            }
        }
        let mut skills: Vec<AttributedSkill> = Vec::new();
        for (_, skill, _) in uses.iter().filter(|(k, _, kind)| k == key && *kind == SkillKind::Direct) {
            if !skills.iter().any(|s| &s.skill == skill) {
                skills.push(AttributedSkill { skill: skill.clone(), how: How::Direct });
                chain = Some(skill.clone());
            }
        }
        if skills.is_empty() {
            if let Some(c) = &chain {
                skills.push(AttributedSkill { skill: c.clone(), how: How::Continued });
            }
        }
        for (_, skill, _) in uses.iter().filter(|(k, _, kind)| k == key && *kind == SkillKind::Command) {
            skills.push(AttributedSkill { skill: skill.clone(), how: How::Command });
        }
        if !skills.is_empty() {
            out.insert(key.clone(), skills);
        }
        last = Some(at);
    }
    out
}
