use std::fs;
use serde::{ Deserialize, Serialize };

/// SphereAMM is the minimal Orbital AMM primitive that keeps *n* token reserves
/// on the surface of a hypersphere with radius `r`. All state-transitions must
/// satisfy the invariant Σ (r − xᵢ)² = r².
#[derive(Clone, Serialize, Deserialize)]
pub struct SphereAMM {
    /// Hypersphere radius `r`.
    pub radius: f64,
    /// Reserves `[x₁, x₂, …, xₙ]` for each token, same order as `token_names`.
    pub reserves: Vec<f64>,
    /// Human-readable token identifiers.
    pub token_names: Vec<String>,
}

impl SphereAMM {
    /// Construct a new SphereAMM from initial reserves. The radius is solved so
    /// that the invariant is satisfied at genesis.
    pub fn new(token_names: Vec<String>, initial_reserves: Vec<f64>) -> Self {
        assert_eq!(
            token_names.len(),
            initial_reserves.len(),
            "token_names and initial_reserves length mismatch"
        );
        let radius = Self::solve_radius(&initial_reserves);
        let amm = Self { radius, reserves: initial_reserves, token_names };
        debug_assert!(amm.check_invariant(), "Invariant not satisfied at construction");
        amm
    }

    /// Calculate the AMM radius `r` that satisfies the invariant for the given
    /// reserves.
    fn solve_radius(reserves: &[f64]) -> f64 {
        if reserves.is_empty() {
            return 0.0;
        }
        let n = reserves.len() as f64;
        let sum_x: f64 = reserves.iter().copied().sum();
        let sum_x2: f64 = reserves
            .iter()
            .map(|x| x * x)
            .sum();
        let a = n - 1.0;
        if a.abs() < 1e-12 {
            // Degenerate n=1 case – radius equals the single reserve.
            return sum_x;
        }
        let b = -2.0 * sum_x;
        let c = sum_x2;
        let disc = b * b - 4.0 * a * c;
        // Numerical robustness – treat tiny negatives as 0.
        let disc_clamped = if disc < 0.0 && disc > -1e-12 { 0.0 } else { disc };
        let sqrt_disc = disc_clamped.sqrt();
        // Only the positive root is meaningful for `r`.
        let r1 = (-b + sqrt_disc) / (2.0 * a);
        let r2 = (-b - sqrt_disc) / (2.0 * a);
        if r1 > 0.0 {
            r1
        } else {
            r2
        }
    }

    /// Verify the hypersphere invariant within a small tolerance.
    pub fn check_invariant(&self) -> bool {
        let lhs: f64 = self.reserves
            .iter()
            .map(|&x| {
                let diff = self.radius - x;
                diff * diff
            })
            .sum();
        (lhs - self.radius * self.radius).abs() < 1e-6
    }

    /// Return the index of a token by name, or an error string if it is absent.
    pub fn index_of(&self, token: &str) -> Result<usize, String> {
        self.token_names
            .iter()
            .position(|t| t == token)
            .ok_or_else(|| format!("Token '{}' not found in pool", token))
    }

    /// Spot price of `to` in units of `from` given by (r − x_to)/(r − x_from).
    pub fn get_spot_price(&self, from: &str, to: &str) -> Result<f64, String> {
        let i = self.index_of(from)?;
        let j = self.index_of(to)?;
        let denom = self.radius - self.reserves[i];
        if denom.abs() < 1e-12 {
            return Err("Division by zero – from-token is at radius".into());
        }
        Ok((self.radius - self.reserves[j]) / denom)
    }

    /// Execute a swap from `from` → `to`, returning the output amount while
    /// keeping the invariant intact.
    pub fn swap(&mut self, from: &str, to: &str, amount_in: f64) -> Result<f64, String> {
        if amount_in <= 0.0 {
            return Err("Swap amount must be positive".into());
        }
        let i = self.index_of(from)?;
        let j = self.index_of(to)?;

        let a = self.reserves[i];
        let b = self.reserves[j];

        // Analytic solution derived from:
        //  (r − (a + Δx))² + (r − (b − Δy))² = (r − a)² + (r − b)²
        //  ⇒ Δy² + 2B Δy + (Δx² − 2A Δx) = 0, where
        //     A = r − a,  B = r − b, Δx = amount_in, Δy = output_amount
        let r = self.radius;
        let A = r - a;
        let B = r - b;
        let c = amount_in * amount_in - 2.0 * A * amount_in;
        let disc = B * B - c;
        if disc < 0.0 {
            return Err("Swap leads to complex solution – probably too large input amount".into());
        }
        let output = -B + disc.sqrt();
        if output <= 0.0 || output > b {
            return Err("Insufficient liquidity for the requested swap".into());
        }

        // Apply state changes.
        self.reserves[i] += amount_in;
        self.reserves[j] -= output;
        debug_assert!(self.check_invariant(), "Invariant broken after swap");
        Ok(output)
    }

    /* ---------- Persistence helpers (CLI convenience) ---------- */
    /// Pretty-print the current pool state.
    pub fn print_state(&self) {
        println!("Sphere AMM State:\n  radius: {}", self.radius);
        for (name, reserve) in self.token_names.iter().zip(&self.reserves) {
            println!("  {}: {}", name, reserve);
        }
        println!("  invariant: {}", if self.check_invariant() { "✓" } else { "✗" });
    }

    pub fn save_state(&self) {
        let json = serde_json::to_string_pretty(self).expect("serialize state");
        fs::write("orbital_pool.json", json).expect("write state");
    }

    pub fn load_state() -> Self {
        match fs::read_to_string("orbital_pool.json") {
            Ok(json) =>
                serde_json::from_str(&json).unwrap_or_else(|_| panic!("invalid state file")),
            Err(_) => panic!("No existing state – initialise first with `init`"),
        }
    }
}

/* ---------- Stand-alone math helpers ---------- */

/// Equal-price point q = r(1 − 1/√n)
pub fn equal_price_point(radius: f64, n_tokens: usize) -> f64 {
    if n_tokens == 0 {
        return 0.0;
    }
    radius * (1.0 - 1.0 / (n_tokens as f64).sqrt())
}

/// Hypersphere invariant value Σ (r − xᵢ)² – r² (should equal 0 when satisfied).
pub fn sphere_invariant(reserves: &[f64], radius: f64) -> f64 {
    let lhs: f64 = reserves
        .iter()
        .map(|&x| {
            let diff = radius - x;
            diff * diff
        })
        .sum();
    lhs - radius * radius
}

/// Decompose reserves into components parallel and orthogonal to the vector
/// v = (1, 1, …, 1)/√n.
/// Returns `(parallel_magnitude, orthogonal_component)`.
pub fn decompose_reserves(reserves: &[f64]) -> (f64, Vec<f64>) {
    let n = reserves.len();
    if n == 0 {
        return (0.0, Vec::new());
    }
    let norm_factor = (n as f64).sqrt();
    let dot = reserves.iter().sum::<f64>(); // dot(reserves, 1)
    let parallel_mag = dot / norm_factor;
    let parallel_component: Vec<f64> = vec![parallel_mag / norm_factor; n];
    let orthogonal_component: Vec<f64> = reserves
        .iter()
        .zip(&parallel_component)
        .map(|(&x, &p)| x - p)
        .collect();
    (parallel_mag, orthogonal_component)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invariant_after_swap() {
        let names = vec!["USDC".into(), "USDT".into()];
        let reserves = vec![100.0, 100.0];
        let mut amm = SphereAMM::new(names, reserves);
        let out = amm.swap("USDC", "USDT", 10.0).unwrap();
        assert!(out > 0.0);
        assert!(amm.check_invariant());
    }
}
