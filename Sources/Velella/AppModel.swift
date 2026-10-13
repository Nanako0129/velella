import Foundation
import Observation
import Security
import VelellaFFI
import VelellaLogin
import VelellaUI

/// Owns the session, its generation and the pending freshly captured token, and executes the
/// Keychain decisions `keychainAction` makes. The token is never observable state: it lives in
/// `@ObservationIgnored` storage only until READY or failure, and never enters `ShellState`.
@MainActor @Observable
final class AppModel {
    var shell: ShellState
    /// Shown as an alert over the main window; never contains a token.
    var notice: String?
    let isDemo: Bool

    @ObservationIgnored private var handle: SessionHandle?
    @ObservationIgnored private var generation: UInt64?
    @ObservationIgnored private var pending: (generation: UInt64, token: String)?

    init(demo: Bool) {
        isDemo = demo
        shell = demo ? .demo : .signedOut
        if !demo { restore() }
    }

    var canSignOut: Bool { !isDemo && shell.isSignedIn }

    /// Called by the login window, once, with the captured token.
    func loginCaptured(_ token: String) {
        guard !isDemo else { return }
        start(token: token, fresh: true)
    }

    func signOut() {
        guard !isDemo else { return }
        handle?.disconnect()
        handle = nil
        generation = nil
        pending = nil
        shell = .signedOut
        notice = nil
        execute(keychainAction(.signedOut, currentGeneration: nil, freshCapture: false))
    }

    // MARK: Session

    private func restore() {
        let (saved, status) = Keychain.load()
        // A read error never deletes (the decision for `.loaded` is always `.none`).
        execute(keychainAction(.loaded(status: status), currentGeneration: generation, freshCapture: false))
        if let saved {
            start(token: saved.token, fresh: false)
        } else if status != errSecItemNotFound {
            notice = "Couldn't read your saved sign-in (error \(status)). Sign in again."
        }
    }

    private func start(token: String, fresh: Bool) {
        handle?.disconnect()
        do {
            let handle = try VelellaFFI.connect(token: token, listener: Listener(model: self))
            self.handle = handle
            generation = handle.generation()
            pending = fresh ? (handle.generation(), token) : nil
            notice = nil
        } catch {
            endSession()
            notice = "That sign-in couldn't be used. Try signing in again."
        }
    }

    fileprivate func receive(_ event: Listener.Event) {
        FileHandle.standardError.write(Data("velella: \(event.name)\n".utf8))
        switch event {
        case .ready(let gen, let userID, let username):
            let fresh = pending?.generation == gen
            let action = keychainAction(.ready(generation: gen), currentGeneration: generation, freshCapture: fresh)
            guard gen == generation else { return }
            shell.currentUser = CurrentUser(
                displayName: username, username: username, avatar: .initials(String(username.prefix(1)).uppercased())
            )
            let token = pending?.token
            pending = nil
            if action == .write, let token {
                execute(.write, token: token, account: String(userID))
            }
        case .authFailed(let gen):
            let action = keychainAction(.authFailed(generation: gen), currentGeneration: generation, freshCapture: false)
            guard gen == generation else { return }
            endSession()
            notice = "Discord didn't accept this sign-in. Sign in again."
            execute(action)
        case .closed(let gen):
            guard gen == generation else { return }
            endSession()
            notice = "Disconnected."
        case .resumed, .reconnecting:
            break
        }
    }

    private func endSession() {
        handle = nil
        generation = nil
        pending = nil
        shell = .signedOut
    }

    // MARK: Keychain

    /// The only place that acts on a `KeychainAction`.
    private func execute(_ action: KeychainAction, token: String? = nil, account: String? = nil) {
        switch action {
        case .write:
            guard let token, let account else { return }
            let status = Keychain.save(token: token, account: account)
            if keychainAction(.saved(status: status), currentGeneration: generation, freshCapture: true) == .sessionOnly {
                // The session keeps running; nothing is written anywhere else.
                notice = "Signed in for this session only: couldn't save your sign-in (error \(status))."
            }
        case .delete:
            let status = Keychain.delete()
            if status != errSecSuccess { notice = "Couldn't remove your saved sign-in (error \(status))." }
        case .sessionOnly, .none:
            break // .sessionOnly is acted on above, where the save status is known.
        }
    }
}

/// Receives session callbacks on a `velella-rt` thread and hands them to the main queue in order.
private final class Listener: SessionListener {
    enum Event {
        case ready(UInt64, UInt64, String)
        case resumed(UInt64)
        case reconnecting(UInt64)
        case authFailed(UInt64)
        case closed(UInt64)

        /// Event name only: no username, id or token reaches stderr.
        var name: String {
            switch self {
            case .ready: "READY"
            case .resumed: "RESUMED"
            case .reconnecting: "RECONNECTING"
            case .authFailed: "AUTH_FAILED"
            case .closed: "CLOSED"
            }
        }
    }

    let model: AppModel
    init(model: AppModel) { self.model = model }

    private func send(_ event: Event) {
        DispatchQueue.main.async { [model] in model.receive(event) }
    }

    func onReady(generation: UInt64, userId: UInt64, username: String) { send(.ready(generation, userId, username)) }
    func onResumed(generation: UInt64) { send(.resumed(generation)) }
    func onReconnecting(generation: UInt64) { send(.reconnecting(generation)) }
    func onAuthFailed(generation: UInt64) { send(.authFailed(generation)) }
    func onClosed(generation: UInt64) { send(.closed(generation)) }
}
