import Security

/// What AppModel does to the saved sign-in. The decision is pure so it can be tested without
/// touching a keychain; AppModel executes it.
public enum KeychainAction: Equatable, Sendable {
    /// Delete-then-add the pending fresh token.
    case write
    /// Remove the saved sign-in (not-found counts as success).
    case delete
    case none
    /// The save failed: keep the session, tell the user it lasts for this session only.
    case sessionOnly
}

public enum KeychainEvent: Equatable, Sendable {
    case ready(generation: UInt64)
    case authFailed(generation: UInt64)
    /// Result of the launch-time read.
    case loaded(status: OSStatus)
    /// Result of executing `.write`.
    case saved(status: OSStatus)
    case signedOut
}

/// - Parameters:
///   - currentGeneration: the session AppModel currently owns; nil after Sign Out or before any
///     connect. Events from any other generation are stale and never touch the keychain.
///   - freshCapture: the READY belongs to a token captured by the login window in this session
///     (a token loaded from the keychain is never re-written).
public func keychainAction(_ event: KeychainEvent, currentGeneration: UInt64?, freshCapture: Bool) -> KeychainAction {
    switch event {
    case .ready(let generation):
        return generation == currentGeneration && freshCapture ? .write : .none
    case .authFailed(let generation):
        return generation == currentGeneration ? .delete : .none
    case .loaded:
        // Found → connect; not found → login; any other error → login with an error. Never delete:
        // a locked or unreachable keychain says nothing about whether the token is still good.
        return .none
    case .saved(let status):
        return status == errSecSuccess ? .none : .sessionOnly
    case .signedOut:
        return .delete
    }
}
