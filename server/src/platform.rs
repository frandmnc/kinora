//! Lo que cambia entre sistemas (macOS y Linux por un lado, Windows por el otro), junto y con
//! la misma forma para los dos: el resto del servidor no pregunta en qué sistema corre. Cada
//! compilación toma solo su versión.

use std::path::{Path, PathBuf};

/// La carpeta del usuario: `HOME` en macOS y Linux, `USERPROFILE` en Windows.
pub fn home_dir() -> Option<PathBuf> {
    std::env::home_dir()
}

/// Espera el pedido de apagarse y devuelve de dónde vino. Además de lo propio de cada sistema,
/// con `KINORA_STOP_ON_STDIN_EOF=1` se apaga cuando se cierra su entrada: así lo apaga la app
/// que lo arrancó (la de la bandeja de Windows), que no tiene cómo mandarle una señal.
pub async fn shutdown_signal() -> &'static str {
    tokio::select! {
        reason = system_signal() => reason,
        reason = stdin_closed() => reason,
    }
}

#[cfg(unix)]
async fn system_signal() -> &'static str {
    use tokio::signal::unix::{SignalKind, signal};
    let mut terminate = signal(SignalKind::terminate()).expect("could not listen for SIGTERM");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => "Ctrl+C",
        _ = terminate.recv() => "SIGTERM",
    }
}

#[cfg(windows)]
async fn system_signal() -> &'static str {
    use tokio::signal::windows::{ctrl_close, ctrl_shutdown};
    let mut close = ctrl_close().expect("could not listen for the console closing");
    let mut shutdown = ctrl_shutdown().expect("could not listen for Windows shutting down");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => "Ctrl+C",
        _ = close.recv() => "console closed",
        _ = shutdown.recv() => "Windows shutting down",
    }
}

async fn stdin_closed() -> &'static str {
    if std::env::var_os("KINORA_STOP_ON_STDIN_EOF").is_none() {
        return std::future::pending().await;
    }
    use tokio::io::AsyncReadExt;
    let mut stdin = tokio::io::stdin();
    let mut buffer = [0u8; 64];
    while matches!(stdin.read(&mut buffer).await, Ok(n) if n > 0) {}
    "stdin closed"
}

/// ffmpeg con prioridad baja: la conversión usa todos los núcleos, y ver algo tiene que ir
/// primero. En macOS y Linux, `nice -n 10 ffmpeg` (nice se reemplaza por ffmpeg: el pid es el
/// de ffmpeg); en Windows, la prioridad "por debajo de lo normal".
pub fn low_priority_ffmpeg() -> tokio::process::Command {
    #[cfg(unix)]
    {
        let mut command = tokio::process::Command::new("nice");
        command.args(["-n", "10", "ffmpeg"]);
        command
    }
    #[cfg(windows)]
    {
        const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut command = tokio::process::Command::new("ffmpeg");
        command.creation_flags(BELOW_NORMAL_PRIORITY_CLASS | CREATE_NO_WINDOW);
        command
    }
}

/// Las raíces para elegir carpeta: los discos montados en macOS (/Volumes) y Linux (/media,
/// /mnt); las unidades (C:\, D:\…) en Windows.
pub fn drive_roots() -> Vec<(String, PathBuf)> {
    #[cfg(unix)]
    {
        ["/Volumes", "/media", "/mnt"]
            .iter()
            .flat_map(|mounts| std::fs::read_dir(mounts).into_iter().flatten().flatten())
            .filter(|entry| entry.file_type().is_ok_and(|t| t.is_dir()))
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                (!name.starts_with('.')).then(|| (name, entry.path()))
            })
            .collect()
    }
    #[cfg(windows)]
    {
        ('A'..='Z')
            .map(|letter| PathBuf::from(format!("{letter}:\\")))
            .filter(|root| root.is_dir())
            .map(|root| (root.to_string_lossy().into_owned(), root))
            .collect()
    }
}

/// Abre una carpeta en el explorador de archivos de esta compu: el Finder en la Mac, el
/// Explorador en Windows y el que corresponda en Linux. No espera a que se cierre.
pub fn reveal_folder(path: &Path) -> std::io::Result<()> {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    std::process::Command::new(program)
        .arg(path)
        .spawn()
        .map(|_| ())
}

/// Si dos rutas están en el mismo disco (y entonces mover es renombrar, instantáneo). En macOS y
/// Linux, el identificador del sistema de archivos; en Windows, la unidad o la carpeta de red.
pub fn same_disk(a: &Path, b: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        match (a.metadata(), b.metadata()) {
            (Ok(a), Ok(b)) => a.dev() == b.dev(),
            _ => false,
        }
    }
    #[cfg(windows)]
    {
        use std::path::Component;
        let volume = |path: &Path| match path.components().next() {
            Some(Component::Prefix(prefix)) => {
                Some(prefix.as_os_str().to_string_lossy().to_uppercase())
            }
            _ => None,
        };
        volume(a).is_some() && volume(a) == volume(b)
    }
}
