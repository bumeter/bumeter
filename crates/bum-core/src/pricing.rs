//! Model pricing table (spec section 4.5).
use std::collections::BTreeMap;

use serde::Deserialize;

use crate::model::Usage;

const DEFAULT_TOML: &str = include_str!("../../../pricing.default.toml");

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct ModelPrice {
    pub input: f64,
    pub cache_write_5m: f64,
    pub cache_write_1h: f64,
    pub cache_read: f64,
    pub output: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Pricing {
    models: BTreeMap<String, ModelPrice>,
}

impl Pricing {
    pub fn default_table() -> Self {
        Self::from_toml(DEFAULT_TOML).expect("pricing.default.toml is valid")
    }

    pub fn default_toml() -> &'static str {
        DEFAULT_TOML
    }

    pub fn from_toml(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }

    /// Exact match; if none, the longest matching prefix.
    fn price(&self, model: &str) -> Option<&ModelPrice> {
        self.models.get(model).or_else(|| {
            self.models.iter().filter(|(k, _)| model.starts_with(k.as_str())).max_by_key(|(k, _)| k.len()).map(|(_, p)| p)
        })
    }

    pub fn cost(&self, model: &str, u: &Usage) -> Option<f64> {
        let p = self.price(model)?;
        let m = |n: u64, price: f64| n as f64 / 1_000_000.0 * price;
        Some(m(u.input, p.input) + m(u.cache_write_5m, p.cache_write_5m) + m(u.cache_write_1h, p.cache_write_1h)
            + m(u.cache_read, p.cache_read) + m(u.output + u.reasoning, p.output))
    }

    /// Total cost and the list of models without a price.
    pub fn cost_of(&self, per_model: &[(String, Usage)]) -> (f64, Vec<String>) {
        let mut total = 0.0;
        let mut unpriced = Vec::new();
        for (model, u) in per_model {
            match self.cost(model, u) {
                Some(c) => total += c,
                None => unpriced.push(model.clone()),
            }
        }
        (total, unpriced)
    }
}

impl Pricing {
    /// Read the user's `pricing.toml`; if absent, write the default so they can edit it.
    pub fn load_or_init(path: &std::path::Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(s) => Self::from_toml(&s).unwrap_or_else(|_| Self::default_table()),
            Err(_) => {
                let _ = std::fs::write(path, DEFAULT_TOML);
                Self::default_table()
            }
        }
    }
}
