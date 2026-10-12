use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

/// Configuración leída de `config.toml`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default = "default_bind")]
    pub bind: IpAddr,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
    #[serde(default)]
    pub import_sources: Vec<PathBuf>,
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
    /// Lee el archivo, crea la carpeta de datos y avisa si falta algún origen.
    pub fn load(path: &Path) -> Result<Config> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("no se pudo leer {}", path.display()))?;
        let config =
            Config::parse(&text).with_context(|| format!("{} tiene un error", path.display()))?;

        fs::create_dir_all(&config.data_dir).with_context(|| {
            format!(
                "no se pudo crear la carpeta de datos {}",
                config.data_dir.display()
            )
        })?;

        for source in &config.import_sources {
            if !source.is_dir() {
                eprintln!("Aviso: la carpeta de origen {} no existe", source.display());
            }
        }

        Ok(config)
    }

    /// Convierte el texto TOML en un `Config` y expande `~` en las rutas.
    fn parse(text: &str) -> Result<Config> {
        let mut config: Config = toml::from_str(text)?;

        config.data_dir = expand_home(&config.data_dir)?;
        config.import_sources = config
            .import_sources
            .iter()
            .map(|path| expand_home(path))
            .collect::<Result<Vec<_>>>()?;

        Ok(config)
    }
}

/// Reemplaza un `~` inicial por la carpeta del usuario.
pub fn expand_home(path: &Path) -> Result<PathBuf> {
    match path.strip_prefix("~") {
        Ok(rest) => {
            let home = std::env::var("HOME").context("la variable HOME no está definida")?;
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
        assert!(config.import_sources.is_empty());
    }

    #[test]
    fn expands_home_in_paths() {
        let home = std::env::var("HOME").unwrap();
        let config =
            Config::parse(r#"import_sources = ["~/Downloads", "/Volumes/Disco"]"#).unwrap();
        assert_eq!(config.data_dir, PathBuf::from(&home).join(".kinora"));
        assert_eq!(
            config.import_sources[0],
            PathBuf::from(&home).join("Downloads")
        );
        assert_eq!(config.import_sources[1], PathBuf::from("/Volumes/Disco"));
    }

    #[test]
    fn rejects_unknown_keys() {
        assert!(Config::parse("puerto = 8090").is_err());
    }
}
