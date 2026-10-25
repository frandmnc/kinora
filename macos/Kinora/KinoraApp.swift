// Kinora en la barra de menú, como Plex: la app es el servidor. Al abrirla arranca Kinora, al
// salir se apaga. Sin ícono en el Dock (LSUIElement en Info.plist).

import AppKit
import SwiftUI

@main
struct KinoraApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate
    @StateObject private var server = Server.shared

    var body: some Scene {
        MenuBarExtra {
            KinoraMenu(server: server)
        } label: {
            Image(systemName: symbol)
        }
    }

    /// Lleno cuando responde, vacío mientras arranca, un aviso si no anda.
    private var symbol: String {
        switch server.state {
        case .running, .external: "play.rectangle.fill"
        case .starting, .updating: "play.rectangle"
        case .stopped, .failed: "exclamationmark.triangle"
        }
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationWillTerminate(_ notification: Notification) {
        MainActor.assumeIsolated { Server.shared.stopForQuit() }
    }
}

struct KinoraMenu: View {
    @ObservedObject var server: Server

    var body: some View {
        Text(status)

        Divider()

        Button("Abrir el Player") { open("") }
            .disabled(!server.isUp)
        Button("Abrir el Admin") { open("admin/") }
            .disabled(!server.isUp)
        if let address = Server.localAddress() {
            let url = "http://\(address):\(server.port)"
            Button("Copiar la dirección para la TV: \(url)") {
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(url, forType: .string)
            }
        }

        Divider()

        Button(server.ownsProcess || server.state == .starting ? "Reiniciar Kinora" : "Prender Kinora") {
            server.restart()
        }
        .disabled(server.state == .external || server.isUpdating)
        Button("Ver los registros") {
            NSWorkspace.shared.open(server.logURL)
        }
        Toggle(
            "Abrir al iniciar sesión",
            isOn: Binding(get: { server.openAtLogin }, set: { server.setOpenAtLogin($0) })
        )

        Divider()

        Button("Salir de Kinora") { NSApp.terminate(nil) }
            .keyboardShortcut("q")
    }

    private var status: String {
        switch server.state {
        case .running: "Kinora \(server.version ?? "") está prendido"
        case .external: "Kinora ya estaba prendido en otro proceso"
        case .starting: "Kinora está arrancando…"
        case .stopped: "Kinora está apagado"
        case .updating(let version): "Instalando Kinora \(version)…"
        case .failed(let reason): "Kinora no arrancó: \(reason)"
        }
    }

    private func open(_ path: String) {
        NSWorkspace.shared.open(server.baseURL.appendingPathComponent(path))
    }
}
