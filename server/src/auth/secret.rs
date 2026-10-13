//! Contraseñas y tokens: lo único que toca criptografía.

use argon2::Argon2;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use sha2::{Digest, Sha256};

/// Argon2id con sal aleatoria, en formato PHC ("$argon2id$v=19$m=…"): la sal y los parámetros
/// van dentro del texto, así que alcanza con guardar eso.
pub fn hash_password(password: &str) -> anyhow::Result<String> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|hash| hash.to_string())
        .map_err(|err| anyhow::anyhow!("could not hash the password: {err}"))
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    Argon2::default()
        .verify_password(password.as_bytes(), hash)
        .is_ok()
}

/// Un token de sesión: 32 bytes al azar, en hexadecimal (64 caracteres).
pub fn new_token() -> String {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("the system random generator failed");
    hex(&bytes)
}

/// Lo que se guarda de un token: su SHA-256. Si alguien copia la base, no le sirve para entrar.
pub fn token_hash(token: &str) -> String {
    hex(&Sha256::digest(token.as_bytes()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passwords_and_tokens() {
        let hash = hash_password("una clave larga").unwrap();
        assert!(hash.starts_with("$argon2id$"));
        assert!(verify_password("una clave larga", &hash));
        assert!(!verify_password("otra clave", &hash));

        let (a, b) = (new_token(), new_token());
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
        assert_eq!(token_hash(&a), token_hash(&a));
        assert_ne!(token_hash(&a), a);
    }
}
