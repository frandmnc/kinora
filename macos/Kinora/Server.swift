// El servidor de Kinora como proceso hijo de la app: lo arranca, lo vuelve a levantar si se cae,
// pregunta cada pocos segundos si responde y lo apaga al salir.

import AppKit
import Foundation
import ServiceManagement

@MainActor
final class Server: ObservableObject {
    static let shared = Server()

    enum State: Equatable {
        case starting
        case running
        /// Responde, pero no lo arrancó la app (por ejemplo, uno de desarrollo en la terminal).
        case external
        case stopped
        case failed(String)
    }

    @Published private(set) var state: State = .stopped
    @Published private(set) var version: String?
    @Published private(set) var openAtLogin = SMAppService.mainApp.status == .enabled

    let port = Server.configuredPort()
    let logURL = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent("Library/Logs/Kinora/kinora.log")

    private var process: Process?
    private var startedAt = Date()
    /// Cuántas veces seguidas se cerró enseguida: a la tercera se deja de insistir.
    private var quickExits = 0
    private var restarting = false
    private var quitting = false
    private var timer: Timer?

    var baseURL: URL { URL(string: "http://localhost:\(port)")! }
    var isUp: Bool { state == .running || state == .external }
    var ownsProcess: Bool { process != nil }

    private init() {
        timer = Timer.scheduledTimer(withTimeInterval: 3, repeats: true) { _ in
            Task { @MainActor in await Server.shared.check() }
        }
        Task { await startUnlessRunning() }
    }

    // MARK: Arrancar y apagar

    /// Si ya hay un Kinora respondiendo en el puerto, no se arranca otro.
    func startUnlessRunning() async {
        if let version = await health() {
            self.version = version
            state = .external
            return
        }
        start()
    }

    func start() {
        guard process == nil else { return }
        let executable = Bundle.main.bundleURL.appendingPathComponent("Contents/MacOS/kinora-server")
        guard let log = openLog() else {
            state = .failed("no se pudo abrir el registro")
            return
        }

        let process = Process()
        process.executableURL = executable
        // Sin argumentos: usa ~/.kinora/config.toml si existe, o los valores por defecto.
        process.currentDirectoryURL = URL(fileURLWithPath: "/")
        var environment = ProcessInfo.processInfo.environment
        // Una app abierta desde el Finder no tiene el PATH de la terminal: sin esto no encuentra
        // ffmpeg ni ffprobe de Homebrew.
        environment["PATH"] = "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
        process.environment = environment
        process.standardOutput = log
        process.standardError = log
        process.terminationHandler = { finished in
            let status = finished.terminationStatus
            Task { @MainActor in await Server.shared.exited(status) }
        }

        do {
            try process.run()
        } catch {
            state = .failed(error.localizedDescription)
            return
        }
        self.process = process
        startedAt = Date()
        state = .starting
    }

    /// Lo apaga y lo vuelve a arrancar.
    func restart() {
        guard let process else {
            quickExits = 0
            start()
            return
        }
        restarting = true
        state = .starting
        process.terminate()
    }

    /// Al salir de la app: SIGTERM (Kinora corta el ffmpeg de la cola y cierra la base) y hasta
    /// 5 segundos de espera.
    func stopForQuit() {
        quitting = true
        guard let process, process.isRunning else { return }
        process.terminate()
        let deadline = Date().addingTimeInterval(5)
        while process.isRunning && Date() < deadline {
            usleep(50_000)
        }
        if process.isRunning {
            kill(process.processIdentifier, SIGKILL)
        }
    }

    private func exited(_ status: Int32) async {
        process = nil
        if quitting { return }
        if restarting {
            restarting = false
            start()
            return
        }
        // Si sigue respondiendo, el puerto lo tiene otro Kinora: este no pudo arrancar.
        if let version = await health() {
            self.version = version
            state = .external
            return
        }
        let quick = Date().timeIntervalSince(startedAt) < 15
        quickExits = quick ? quickExits + 1 : 0
        if quickExits >= 3 {
            state = .failed("se cerró enseguida \(quickExits) veces (código \(status))")
            return
        }
        state = .starting
        try? await Task.sleep(nanoseconds: 3_000_000_000)
        if process == nil && !quitting { start() }
    }

    // MARK: Estado

    func check() async {
        let version = await health()
        self.version = version
        switch (version != nil, process != nil) {
        case (true, true):
            state = .running
        case (true, false):
            state = .external
        case (false, true):
            // Arrancando, o colgado: no responde.
            if state == .running { state = .starting }
        case (false, false):
            // El de afuera se apagó: lo reemplaza el de la app.
            if state == .external { start() }
        }
    }

    /// GET /api/health: la versión si responde, `nil` si no.
    private func health() async -> String? {
        var request = URLRequest(url: baseURL.appendingPathComponent("api/health"))
        request.timeoutInterval = 2
        request.cachePolicy = .reloadIgnoringLocalCacheData
        guard let (data, response) = try? await URLSession.shared.data(for: request),
              (response as? HTTPURLResponse)?.statusCode == 200,
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else { return nil }
        return json["version"] as? String ?? "?"
    }

    // MARK: Abrir al iniciar sesión

    func setOpenAtLogin(_ enabled: Bool) {
        do {
            if enabled {
                try SMAppService.mainApp.register()
            } else {
                try SMAppService.mainApp.unregister()
            }
        } catch {
            let alert = NSAlert()
            alert.messageText = "No se pudo cambiar \"Abrir al iniciar sesión\""
            alert.informativeText = error.localizedDescription
            alert.runModal()
        }
        let status = SMAppService.mainApp.status
        if status == .requiresApproval {
            SMAppService.openSystemSettingsLoginItems()
        }
        openAtLogin = status == .enabled
    }

    // MARK: Ayudas

    /// El puerto de ~/.kinora/config.toml (`port = 8090`), o 8090.
    private static func configuredPort() -> Int {
        let config = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".kinora/config.toml")
        guard let text = try? String(contentsOf: config, encoding: .utf8) else { return 8090 }
        for line in text.split(separator: "\n") {
            let parts = line.split(separator: "#", maxSplits: 1)[0].split(separator: "=", maxSplits: 1)
            if parts.count == 2,
               parts[0].trimmingCharacters(in: .whitespaces) == "port",
               let port = Int(parts[1].trimmingCharacters(in: .whitespaces)) {
                return port
            }
        }
        return 8090
    }

    /// El archivo de registro, para agregar al final. Si pasa de 10 MB, el anterior queda como
    /// kinora.log.1.
    private func openLog() -> FileHandle? {
        let files = FileManager.default
        try? files.createDirectory(at: logURL.deletingLastPathComponent(), withIntermediateDirectories: true)
        if let size = (try? files.attributesOfItem(atPath: logURL.path))?[.size] as? Int, size > 10_000_000 {
            let old = logURL.appendingPathExtension("1")
            try? files.removeItem(at: old)
            try? files.moveItem(at: logURL, to: old)
        }
        if !files.fileExists(atPath: logURL.path) {
            files.createFile(atPath: logURL.path, contents: nil)
        }
        guard let handle = try? FileHandle(forWritingTo: logURL) else { return nil }
        handle.seekToEndOfFile()
        return handle
    }

    /// La dirección de esta compu en la red de la casa (IPv4 de Wi-Fi o Ethernet), para la TV.
    static func localAddress() -> String? {
        var list: UnsafeMutablePointer<ifaddrs>?
        guard getifaddrs(&list) == 0, let first = list else { return nil }
        defer { freeifaddrs(list) }
        var found: [(name: String, address: String)] = []
        for pointer in sequence(first: first, next: { $0.pointee.ifa_next }) {
            let entry = pointer.pointee
            guard let address = entry.ifa_addr, address.pointee.sa_family == UInt8(AF_INET) else { continue }
            let name = String(cString: entry.ifa_name)
            guard name.hasPrefix("en") else { continue }
            var host = [CChar](repeating: 0, count: Int(NI_MAXHOST))
            if getnameinfo(address, socklen_t(address.pointee.sa_len), &host, socklen_t(host.count), nil, 0, NI_NUMERICHOST) == 0 {
                found.append((name, String(cString: host)))
            }
        }
        return (found.first { $0.name == "en0" } ?? found.first)?.address
    }
}
