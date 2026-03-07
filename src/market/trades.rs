use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub instrument_name: String,
    pub price: f64,
    pub amount: f64,
    pub direction: String,
    pub timestamp: i64,
    pub trade_id: String,
}
