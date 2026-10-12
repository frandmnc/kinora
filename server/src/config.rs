use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

/// Configuración leída de `config.toml`: lo que hace falta antes de abrir la base de datos.
/// El resto (como la carpeta media) se configura desde el Admin.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default = "default_bind")]
    pub bind: IpAddr,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
}

fn default_bind() -> IpAddr {
    IpAddr::from([0, 0, 0, 0])
}

fn default_port() -> u16 {
    8090
}

fn default_data_dir() -> PathBuf {
    PathBuf::from("~/.kinora")
}

impl Config {
    /// Lee el archivo y crea la carpeta de datos.
    pub fn load(path: &Path) -> Result<Config> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("could not read {}", path.display()))?;
        let config =
            Config::parse(&text).with_context(|| format!("{} is not valid", path.display()))?;

        fs::create_dir_all(&config.data_dir).with_context(|| {
            format!(
                "could not create the data folder {}",
                config.data_dir.display()
            )
        })?;

        Ok(config)
    }

    /// Convierte el texto TOML en un `Config` y expande `~` en las rutas.
    fn parse(text: &str) -> Result<Config> {
        let mut config: Config = toml::from_str(text)?;

        config.data_dir = expand_home(&config.data_dir)?;

        Ok(config)
    }
}

/// Reemplaza un `~` inicial por la carpeta del usuario.
pub fn expand_home(path: &Path) -> Result<PathBuf> {
    match path.strip_prefix("~") {
        Ok(rest) => {
            let home = std::env::var("HOME").context("the HOME environment variable is not set")?;
            Ok(PathBuf::from(home).join(rest))
        }
        Err(_) => Ok(path.to_path_buf()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_file_uses_defaults() {
        let config = Config::parse("").unwrap();
        assert_eq!(config.port, 8090);
        assert_eq!(config.bind.to_string(), "0.0.0.0");
        let home = std::env::var("HOME").unwrap();
        assert_eq!(config.data_dir, PathBuf::from(home).join(".kinora"));
    }

    #[test]
    fn rejects_unknown_keys() {
        assert!(Config::parse("puerto = 8090").is_err());
    }
}
