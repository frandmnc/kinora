//! Freno a los intentos de contraseña: con Kinora expuesto a internet (Funnel), sin esto
//! alguien podría probar contraseñas sin parar.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Intentos fallidos que se toleran por origen dentro de la ventana.
const MAX_FAILURES: usize = 5;
const WINDOW: Duration = Duration::from_secs(15 * 60);

#[derive(Default)]
pub struct LoginLimiter {
    failures: Mutex<HashMap<String, Vec<Instant>>>,
}

impl LoginLimiter {
    /// Si `client` puede intentar ahora; si no, cuánto falta para que pueda.
    pub fn check(&self, client: &str) -> Result<(), Duration> {
        let mut failures = self.failures.lock().unwrap();
        let recent = failures.entry(client.to_string()).or_default();
        recent.retain(|at| at.elapsed() < WINDOW);
        if recent.len() >= MAX_FAILURES {
            return Err(WINDOW - recent[0].elapsed());
        }
        Ok(())
    }

    pub fn fail(&self, client: &str) {
        let mut failures = self.failures.lock().unwrap();
        failures
            .entry(client.to_string())
            .or_default()
            .push(Instant::now());
    }

    /// Al entrar bien, se olvidan los fallos de ese origen.
    pub fn succeed(&self, client: &str) {
        self.failures.lock().unwrap().remove(client);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_after_five_failures_per_client() {
        let limiter = LoginLimiter::default();
        for _ in 0..MAX_FAILURES {
            assert!(limiter.check("1.2.3.4").is_ok());
            limiter.fail("1.2.3.4");
        }
        assert!(limiter.check("1.2.3.4").is_err());
        assert!(
            limiter.check("5.6.7.8").is_ok(),
            "otro origen no se ve afectado"
        );
        limiter.succeed("1.2.3.4");
        assert!(limiter.check("1.2.3.4").is_ok());
    }
}
