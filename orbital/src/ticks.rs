use std::collections::HashMap;
use std::fs;

use serde::{ Deserialize, Serialize };

use crate::sphere::{ decompose_reserves, SphereAMM };

/// A single liquidity band ("tick") of the Orbital AMM.
#[derive(Clone, Serialize, Deserialize)]
pub struct OrbitalTick {
    pub sphere_amm: SphereAMM,
    /// Constant `c` defining the plane r_parallel = c that bounds this tick.
    pub plane_constant: f64,
    /// LP ownership mapping – **not** optimized, but fine for simulation.
    pub lp_shares: HashMap<String, f64>,
}

impl OrbitalTick {
    /// Convenience constructor from raw reserves and plane constant.
    pub fn new(token_names: Vec<String>, reserves: Vec<f64>, plane_constant: f64) -> Self {
        let amm = SphereAMM::new(token_names, reserves);
        Self { sphere_amm: amm, plane_constant, lp_shares: HashMap::new() }
    }

    /// Parallel component magnitude of the current reserves vector.
    fn parallel_magnitude(&self) -> f64 {
        let (mag, _) = decompose_reserves(&self.sphere_amm.reserves);
        mag
    }

    pub fn is_interior(&self) -> bool {
        self.parallel_magnitude() > self.plane_constant + 1e-6
    }

    pub fn is_boundary(&self) -> bool {
        (self.parallel_magnitude() - self.plane_constant).abs() < 1e-6
    }

    /// Add liquidity amounts for an LP. Very simplified: shares are proportional
    /// to the sum of the deposited token amounts.
    pub fn add_liquidity(&mut self, lp_id: &str, amounts: &[f64]) -> Result<(), String> {
        if amounts.len() != self.sphere_amm.reserves.len() {
            return Err("Amounts length mismatch".into());
        }
        for (r, a) in self.sphere_amm.reserves.iter_mut().zip(amounts.iter()) {
            *r += *a;
        }
        // Re-solve radius to respect the invariant (keeping deposits on sphere).
        let radius = crate::sphere::sphere_invariant(&self.sphere_amm.reserves, 0.0); // placeholder call just to access fn
        let _ = radius; // avoid warning
        self.sphere_amm.radius = {
            // duplicate logic of solve_radius – small DRY violation for privacy.
            let n = self.sphere_amm.reserves.len() as f64;
            let sum_x: f64 = self.sphere_amm.reserves.iter().copied().sum();
            let sum_x2: f64 = self.sphere_amm.reserves
                .iter()
                .map(|x| x * x)
                .sum();
            let a = n - 1.0;
            if a.abs() < 1e-12 {
                sum_x
            } else {
                let b = -2.0 * sum_x;
                let c = sum_x2;
                let disc = (b * b - 4.0 * a * c).max(0.0);
                let r1 = (-b + disc.sqrt()) / (2.0 * a);
                if r1 > 0.0 {
                    r1
                } else {
                    (-b - disc.sqrt()) / (2.0 * a)
                }
            }
        };
        // Update shares
        let share_delta: f64 = amounts.iter().sum();
        *self.lp_shares.entry(lp_id.to_string()).or_default() += share_delta;
        Ok(())
    }

    /// Withdraw a percentage (0..=1) of the LP's position. Returns withdrawn
    /// amounts per token.
    pub fn withdraw_liquidity(&mut self, lp_id: &str, percentage: f64) -> Result<Vec<f64>, String> {
        if !(0.0..=1.0).contains(&percentage) {
            return Err("percentage must be in [0,1]".into());
        }
        let user_shares = self.lp_shares
            .get(lp_id)
            .ok_or_else(|| "LP id not found".to_string())?
            .to_owned();
        if user_shares == 0.0 {
            return Err("LP has no shares".into());
        }
        let total_shares: f64 = self.lp_shares.values().copied().sum();
        let shares_to_remove = user_shares * percentage;
        let ratio = shares_to_remove / total_shares;
        // Withdraw proportional amounts
        let mut withdrawn = Vec::with_capacity(self.sphere_amm.reserves.len());
        for r in self.sphere_amm.reserves.iter_mut() {
            let amt = *r * ratio;
            *r -= amt;
            withdrawn.push(amt);
        }
        // Recompute radius
        self.sphere_amm.radius = {
            let n = self.sphere_amm.reserves.len() as f64;
            let sum_x: f64 = self.sphere_amm.reserves.iter().copied().sum();
            let sum_x2: f64 = self.sphere_amm.reserves
                .iter()
                .map(|x| x * x)
                .sum();
            let a = n - 1.0;
            if a.abs() < 1e-12 {
                sum_x
            } else {
                let b = -2.0 * sum_x;
                let c = sum_x2;
                let disc = (b * b - 4.0 * a * c).max(0.0);
                let r1 = (-b + disc.sqrt()) / (2.0 * a);
                if r1 > 0.0 {
                    r1
                } else {
                    (-b - disc.sqrt()) / (2.0 * a)
                }
            }
        };
        // Update shares bookkeeping
        if percentage >= 1.0 - 1e-12 {
            self.lp_shares.remove(lp_id);
        } else {
            *self.lp_shares.get_mut(lp_id).unwrap() -= shares_to_remove;
        }
        Ok(withdrawn)
    }

    /// Total liquidity proxy (sum of reserves).
    pub fn liquidity(&self) -> f64 {
        self.sphere_amm.reserves.iter().sum()
    }
}

/* ------------------------------------------------------------- */

#[derive(Clone, Serialize, Deserialize)]
pub struct MultiTickAMM {
    pub ticks: Vec<OrbitalTick>,
    pub global_reserves: Vec<f64>,
    pub token_names: Vec<String>,
}

impl MultiTickAMM {
    pub fn new(token_names: Vec<String>) -> Self {
        let m = token_names.len();
        Self { ticks: Vec::new(), global_reserves: vec![0.0; m], token_names }
    }

    /// Recompute the global reserve vector from constituent ticks.
    fn recompute_global_reserves(&mut self) {
        self.global_reserves.fill(0.0);
        for tick in &self.ticks {
            for (g, r) in self.global_reserves.iter_mut().zip(&tick.sphere_amm.reserves) {
                *g += *r;
            }
        }
    }

    pub fn add_tick(&mut self, plane_constant: f64, reserves: Vec<f64>) {
        assert_eq!(reserves.len(), self.token_names.len(), "reserve length mismatch");
        let tick = OrbitalTick::new(self.token_names.clone(), reserves, plane_constant);
        self.ticks.push(tick);
        self.recompute_global_reserves();
    }

    /// Classify ticks into interior and boundary indices.
    pub fn classify_ticks(&self) -> (Vec<usize>, Vec<usize>) {
        let mut interior = Vec::new();
        let mut boundary = Vec::new();
        for (idx, tick) in self.ticks.iter().enumerate() {
            if tick.is_interior() {
                interior.push(idx);
            } else if tick.is_boundary() {
                boundary.push(idx);
            }
        }
        (interior, boundary)
    }

    /// Very naive routing: route through ticks in ascending plane_constant order
    /// until the amount is fully executed.
    pub fn route_trade(&mut self, from: &str, to: &str, mut amount: f64) -> Result<f64, String> {
        let mut total_output = 0.0;
        // Sort tick indices by plane_constant
        let mut idxs: Vec<usize> = (0..self.ticks.len()).collect();
        idxs.sort_unstable_by(|&a, &b|
            self.ticks[a].plane_constant.partial_cmp(&self.ticks[b].plane_constant).unwrap()
        );
        for idx in idxs {
            if amount <= 0.0 {
                break;
            }
            let tick = &mut self.ticks[idx];
            let available = tick.sphere_amm.reserves[tick.sphere_amm.index_of(from)?];
            if available <= 1e-12 {
                continue;
            }
            let trade_in = amount.min(available * 0.9); // keep small buffer
            if trade_in <= 0.0 {
                continue;
            }
            let out = tick.sphere_amm.swap(from, to, trade_in)?;
            amount -= trade_in;
            total_output += out;
        }
        self.recompute_global_reserves();
        if amount > 1e-8 {
            return Err("Not enough liquidity across ticks to satisfy trade".into());
        }
        Ok(total_output)
    }

    /// Aggregated spot price across ticks weighted by token liquidity.
    pub fn get_aggregated_price(&self, from: &str, to: &str) -> Result<f64, String> {
        let mut num = 0.0;
        let mut denom = 0.0;
        for tick in &self.ticks {
            let weight = tick.sphere_amm.reserves[tick.sphere_amm.index_of(from)?];
            if weight == 0.0 {
                continue;
            }
            let price = tick.sphere_amm.get_spot_price(from, to)?;
            num += price * weight;
            denom += weight;
        }
        if denom == 0.0 {
            return Err("No liquidity for base token across ticks".into());
        }
        Ok(num / denom)
    }

    /// Save state to disk in `multi_tick.json`.
    pub fn save_state(&self) {
        let json = serde_json::to_string_pretty(self).expect("serialize");
        fs::write("multi_tick.json", json).expect("write file");
    }

    /// Load state or create empty.
    pub fn load_state(token_names: Vec<String>) -> Self {
        match fs::read_to_string("multi_tick.json") {
            Ok(bytes) => serde_json::from_str(&bytes).unwrap_or_else(|_| Self::new(token_names)),
            Err(_) => Self::new(token_names),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tick_state() {
        let names = vec!["USDC".into(), "USDT".into()];
        let reserves = vec![100.0, 100.0];
        let plane_constant = 50.0;
        let tick = OrbitalTick::new(names, reserves, plane_constant);
        assert!(tick.is_interior());
    }

    #[test]
    fn test_multi_tick_routing() {
        let names = vec!["USDC".into(), "USDT".into()];
        let mut multi = MultiTickAMM::new(names.clone());
        multi.add_tick(50.0, vec![100.0, 100.0]);
        multi.add_tick(70.0, vec![50.0, 50.0]);
        let out = multi.route_trade("USDC", "USDT", 30.0).unwrap();
        assert!(out > 0.0);
    }
}
