use anyhow::{Context, Result};
use rusqlite::Connection;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

use crate::analysis::opportunity::Opportunity;
use crate::events::bus::TickerData;
use crate::market::instruments::Instrument;

pub struct Storage {
    conn: Arc<Mutex<Connection>>,
}

impl Storage {
    pub fn new(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)
            .context("Failed to open SQLite database")?;

        let storage = Storage {
            conn: Arc::new(Mutex::new(conn)),
        };
        storage.initialize()?;
        Ok(storage)
    }

    fn initialize(&self) -> Result<()> {
        let conn = self.conn.blocking_lock();

        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS instruments (
                instrument_name TEXT PRIMARY KEY,
                strike REAL,
                expiration_timestamp INTEGER,
                option_type TEXT,
                is_active BOOLEAN,
                updated_at INTEGER
            );

            CREATE TABLE IF NOT EXISTS tickers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                instrument_name TEXT,
                timestamp INTEGER,
                mark_price REAL,
                mark_iv REAL,
                best_bid_price REAL,
                best_ask_price REAL,
                best_bid_amount REAL,
                best_ask_amount REAL,
                open_interest REAL,
                delta REAL,
                gamma REAL,
                vega REAL,
                theta REAL
            );

            CREATE TABLE IF NOT EXISTS opportunities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                strategy_type TEXT,
                description TEXT,
                expected_profit REAL,
                risk_level TEXT,
                instruments TEXT,
                legs TEXT,
                detected_at INTEGER,
                expired_at INTEGER
            );

            CREATE INDEX IF NOT EXISTS idx_tickers_instrument ON tickers(instrument_name);
            CREATE INDEX IF NOT EXISTS idx_tickers_timestamp ON tickers(timestamp);
            CREATE INDEX IF NOT EXISTS idx_opportunities_type ON opportunities(strategy_type);
            CREATE INDEX IF NOT EXISTS idx_opportunities_detected ON opportunities(detected_at);
            ",
        )
        .context("Failed to create tables")?;

        info!("Database initialized");
        Ok(())
    }

    pub async fn save_instrument(&self, instrument: &Instrument) -> Result<()> {
        let conn = self.conn.lock().await;
        let now = chrono::Utc::now().timestamp();

        conn.execute(
            "INSERT OR REPLACE INTO instruments (instrument_name, strike, expiration_timestamp, option_type, is_active, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                instrument.instrument_name,
                instrument.strike,
                instrument.expiration_timestamp,
                instrument.option_type.to_string(),
                instrument.is_active,
                now,
            ],
        )?;
        Ok(())
    }

    pub async fn save_ticker(
        &self,
        instrument_name: &str,
        data: &TickerData,
    ) -> Result<()> {
        let conn = self.conn.lock().await;

        conn.execute(
            "INSERT INTO tickers (instrument_name, timestamp, mark_price, mark_iv, best_bid_price, best_ask_price, best_bid_amount, best_ask_amount, open_interest, delta, gamma, vega, theta)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            rusqlite::params![
                instrument_name,
                data.timestamp,
                data.mark_price,
                data.mark_iv,
                data.best_bid_price,
                data.best_ask_price,
                data.best_bid_amount,
                data.best_ask_amount,
                data.open_interest,
                data.delta,
                data.gamma,
                data.vega,
                data.theta,
            ],
        )?;
        Ok(())
    }

    pub async fn save_opportunity(&self, opp: &Opportunity) -> Result<()> {
        let conn = self.conn.lock().await;
        let instruments_json = serde_json::to_string(&opp.instruments)?;
        let legs_json = serde_json::to_string(&opp.legs)?;

        conn.execute(
            "INSERT INTO opportunities (strategy_type, description, expected_profit, risk_level, instruments, detected_at, legs)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                opp.strategy_type,
                opp.description,
                opp.expected_profit,
                opp.risk_level.to_string(),
                instruments_json,
                opp.detected_at,
                legs_json,
            ],
        )?;
        Ok(())
    }
}

impl Clone for Storage {
    fn clone(&self) -> Self {
        Storage {
            conn: self.conn.clone(),
        }
    }
}
