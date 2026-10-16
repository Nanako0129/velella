import Security

/// What AppModel does with a token core refreshed from READY `auth_token` (menu plan ID, T-8,
/// I-2). Pure, like `keychainAction`, so it is tested without a keychain.
public enum TokenRefreshAction: Equatable, Sendable {
    /// The fresh capture of this generation is not written yet: the refreshed token replaces it,
    /// so only the refreshed one is written at `.ready`.
    case replacePending
    /// Update the saved sign-in of this account in place.
    case update(account: String)
    case none
}

/// - Parameters:
///   - currentGeneration: the session AppModel owns (nil after Sign Out): any other generation's
///     refresh is stale and never touches the keychain.
///   - pendingGeneration: the generation of the freshly captured token not yet written.
///   - storedAccount: the account of the saved sign-in (`Keychain.Saved.account`, or the one
///     just written); nil when nothing is saved. A refresh for another account writes nothing.
public func tokenRefreshAction(
    generation: UInt64, userID: String, currentGeneration: UInt64?, pendingGeneration: UInt64?,
    storedAccount: String?
) -> TokenRefreshAction {
    guard generation == currentGeneration else { return .none }
    if pendingGeneration == generation { return .replacePending }
    guard let storedAccount, storedAccount == userID else { return .none }
    return .update(account: storedAccount)
}

/// The keychain operation a refresh needs; AppModel's is `Keychain`, tests use a fake.
public protocol SignInUpdating {
    /// `SecItemUpdate` of the item with this service and account; the status as is.
    func update(token: String, account: String) -> OSStatus
}

/// Executes `action`: replaces `pending` or updates the saved item. Returns the failing status
/// of an update (the old item is kept; the caller shows a notice), nil otherwise.
public func applyTokenRefresh(
    _ action: TokenRefreshAction, token: String, pending: inout (generation: UInt64, token: String)?,
    keychain: some SignInUpdating
) -> OSStatus? {
    switch action {
    case .replacePending:
        if let generation = pending?.generation { pending = (generation, token) }
        return nil
    case .update(let account):
        let status = keychain.update(token: token, account: account)
        return status == errSecSuccess ? nil : status
    case .none:
        return nil
    }
}
