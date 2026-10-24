// Kinora en la bandeja de Windows, como Plex: Kinora.exe arranca el servidor
// (kinora-server.exe, al lado), lo vuelve a levantar si se cae y lo apaga al salir. El ícono dice
// si responde, y el menú abre el Player y el Admin, copia la dirección para la TV, reinicia y
// muestra los registros. Es el par de la app de la barra de menú de la Mac (macos/Kinora).
//
// `Kinora.exe --quit` le pide al que está abierto que se cierre y espera a que termine: lo usa el
// instalador antes de reemplazar los archivos.

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod app;

#[cfg(windows)]
fn main() {
    app::main();
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Kinora.exe is the Windows tray app: build it with --target x86_64-pc-windows-gnu");
}
