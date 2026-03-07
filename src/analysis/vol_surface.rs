use tracing::info;

use crate::analysis::opportunity::{Opportunity, RiskLevel, TradeLeg};
use crate::market::instruments::{InstrumentRegistry, OptionType};
use crate::market::ticker::TickerCache;

/// Detects anomalies in the implied volatility surface
pub struct VolSurfaceAnalyzer {
    max_iv_jump: f64,
}

impl VolSurfaceAnalyzer {
    pub fn new(max_iv_jump: f64) -> Self {
        VolSurfaceAnalyzer { max_iv_jump }
    }

    pub async fn scan(
        &self,
        registry: &InstrumentRegistry,
        ticker_cache: &TickerCache,
    ) -> Vec<Opportunity> {
        let mut opportunities = Vec::new();
        let expirations = registry.get_expirations().await;

        for expiration in &expirations {
            // (strike, iv, name, bid, ask, vega, underlying_price)
            let mut iv_points: Vec<(f64, f64, String, f64, f64, f64, f64)> = Vec::new();

            let all_instruments = registry.get_all().await;
            for inst in &all_instruments {
                if inst.expiration_timestamp == *expiration && inst.option_type == OptionType::Call {
                    if let Some(ticker) = ticker_cache.get(&inst.instrument_name).await {
                        if ticker.mark_iv > 0.0 {
                            let bid = match ticker.best_bid_price {
                                Some(b) if b > 0.0 => b,
                                _ => continue,
                            };
                            let ask = match ticker.best_ask_price {
                                Some(a) if a > 0.0 => a,
                                _ => continue,
                            };
                            iv_points.push((
                                inst.strike,
                                ticker.mark_iv,
                                inst.instrument_name.clone(),
                                bid,
                                ask,
                                ticker.vega,
                                ticker.underlying_price,
                            ));
                        }
                    }
                }
            }

            iv_points.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

            // IV anomaly jumps
            for window in iv_points.windows(2) {
                let iv_diff = (window[1].1 - window[0].1).abs();
                if iv_diff > self.max_iv_jump {
                    let (high_iv_idx, low_iv_idx) = if window[1].1 > window[0].1 {
                        (1, 0)
                    } else {
                        (0, 1)
                    };

                    // Estimate profit if IV converges 50%
                    // Deribit vega = BTC value change per 1% IV change
                    // Sell high IV (profits from IV drop), buy low IV (profits from IV rise)
                    // Each leg's IV moves ~iv_diff/4 toward convergence (50% total, split between two)
                    let vega_high = window[high_iv_idx].5.abs();
                    let vega_low = window[low_iv_idx].5.abs();
                    let convergence = 0.5; // assume 50% IV convergence
                    let iv_move = iv_diff * convergence;
                    let est_profit_btc = (vega_high + vega_low) * iv_move;
                    let underlying = window[0].6.max(window[1].6);
                    let est_profit_usd = est_profit_btc * underlying;

                    // Cost = net premium paid/received (sell high - buy low)
                    let premium_received = window[high_iv_idx].3; // sell at bid
                    let premium_paid = window[low_iv_idx].4; // buy at ask
                    let net_cost_btc = (premium_paid - premium_received).abs();
                    let total_cost_usd = net_cost_btc * underlying;

                    // Fee: 2 option legs × 0.03%
                    let fee_usd = underlying * 0.0003 * 2.0;

                    let profit_usd = (est_profit_usd - fee_usd).max(0.0);

                    info!(
                        iv_diff = iv_diff,
                        est_profit_usd = profit_usd,
                        "IV surface anomaly between {} and {}",
                        window[0].0, window[1].0
                    );
                    opportunities.push(Opportunity {
                        strategy_type: "vol_surface_anomaly".to_string(),
                        description: format!(
                            "IV jump {:.1}% | {} ({:.1}%) vs {} ({:.1}%) | Sell high IV, buy low IV | ~${:.0} if 50% converge",
                            iv_diff, window[0].0, window[0].1, window[1].0, window[1].1, profit_usd
                        ),
                        legs: vec![
                            TradeLeg::sell(
                                1,
                                &window[high_iv_idx].2,
                                window[high_iv_idx].3,
                                1.0,
                            ),
                            TradeLeg::buy(
                                2,
                                &window[low_iv_idx].2,
                                window[low_iv_idx].4,
                                1.0,
                            ),
                        ],
                        expected_profit: profit_usd,
                        total_cost: total_cost_usd,
                        risk_level: RiskLevel::Medium,
                        instruments: vec![window[0].2.clone(), window[1].2.clone()],
                        detected_at: chrono::Utc::now().timestamp(),
                        expiry_timestamp: Some(*expiration),
                    });
                }
            }

            // Butterfly IV deviation
            for window in iv_points.windows(3) {
                let interpolated = (window[0].1 + window[2].1) / 2.0;
                let deviation = window[1].1 - interpolated;

                if deviation.abs() > self.max_iv_jump * 0.5 {
                    // Estimate profit: middle vega × 2 (qty=2) converging toward interpolated
                    // Wings vega × 1 each moving slightly in opposite direction
                    // Net vega profit ≈ middle_vega * 2 * |deviation| * convergence
                    //                 - wing_vega_avg * |deviation| * convergence
                    let convergence = 0.5;
                    let middle_vega = window[1].5.abs();
                    let wing_vega = (window[0].5.abs() + window[2].5.abs()) / 2.0;
                    let est_profit_btc =
                        (middle_vega * 2.0 - wing_vega * 2.0) * deviation.abs() * convergence;
                    let est_profit_btc = est_profit_btc.max(0.0);
                    let underlying = window[1].6;
                    let est_profit_usd = est_profit_btc * underlying;

                    // Cost: net premium
                    let (legs, desc, net_cost_btc) = if deviation < 0.0 {
                        // Middle IV too low → buy middle, sell wings (long butterfly)
                        let cost = window[1].4 * 2.0 - window[0].3 - window[2].3;
                        (
                            vec![
                                TradeLeg::sell(1, &window[0].2, window[0].3, 1.0),
                                TradeLeg::buy(2, &window[1].2, window[1].4, 2.0),
                                TradeLeg::sell(3, &window[2].2, window[2].3, 1.0),
                            ],
                            "Middle IV cheap → Long butterfly (sell wings, buy middle)",
                            cost.abs(),
                        )
                    } else {
                        // Middle IV too high → sell middle, buy wings (short butterfly)
                        let cost = window[0].4 + window[2].4 - window[1].3 * 2.0;
                        (
                            vec![
                                TradeLeg::buy(1, &window[0].2, window[0].4, 1.0),
                                TradeLeg::sell(2, &window[1].2, window[1].3, 2.0),
                                TradeLeg::buy(3, &window[2].2, window[2].4, 1.0),
                            ],
                            "Middle IV rich → Short butterfly (buy wings, sell middle)",
                            cost.abs(),
                        )
                    };

                    let total_cost_usd = net_cost_btc * underlying;
                    let fee_usd = underlying * 0.0003 * 4.0; // 4 option legs
                    let profit_usd = (est_profit_usd - fee_usd).max(0.0);

                    info!(
                        strike = window[1].0,
                        deviation = deviation,
                        est_profit_usd = profit_usd,
                        "Butterfly IV opportunity"
                    );
                    opportunities.push(Opportunity {
                        strategy_type: "butterfly_spread".to_string(),
                        description: format!(
                            "{} | K={} IV {:.1}% vs interp {:.1}% (dev {:.1}%) | ~${:.0}",
                            desc, window[1].0, window[1].1, interpolated, deviation, profit_usd
                        ),
                        legs,
                        expected_profit: profit_usd,
                        total_cost: total_cost_usd,
                        risk_level: RiskLevel::Medium,
                        instruments: vec![
                            window[0].2.clone(),
                            window[1].2.clone(),
                            window[2].2.clone(),
                        ],
                        detected_at: chrono::Utc::now().timestamp(),
                        expiry_timestamp: Some(*expiration),
                    });
                }
            }
        }

        opportunities
    }
}
