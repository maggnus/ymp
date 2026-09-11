use rand::Rng;
use rand_distr::{Beta, Distribution};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Reputation {
    pub successes: u32,
    pub failures: u32,
}
impl Reputation {
    pub fn mean(&self) -> f64 {
        (1.0 + self.successes as f64) / (2.0 + self.successes as f64 + self.failures as f64)
    }
    pub fn sample(&self, rng: &mut impl Rng) -> f64 {
        Beta::new(1.0 + self.successes as f64, 1.0 + self.failures as f64)
            .expect("positive parameters")
            .sample(rng)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    #[test]
    fn experienced_success_wins_more_but_newcomers_get_opportunities() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(1);
        let experienced = Reputation {
            successes: 80,
            failures: 20,
        };
        let novice = Reputation::default();
        let wins = (0..10000)
            .filter(|_| experienced.sample(&mut rng) > novice.sample(&mut rng))
            .count();
        assert!(wins > 7500 && wins < 8500, "{wins}");
    }
}
