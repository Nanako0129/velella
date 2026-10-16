import Security
import Testing
@testable import VelellaLogin

// Token refresh against a fake keychain (menu plan ID, T-8, I-2). The fake records every write;
// the real Security item APIs are never called from tests (they would act on the user's item).

private final class FakeKeychain: SignInUpdating, @unchecked Sendable {
    var items: [String: String] = [:]
    var writes: [String] = []
    var updateStatus: OSStatus = errSecSuccess

    func save(token: String, account: String) {
        items = [account: token]
        writes.append("save \(token)")
    }

    func update(token: String, account: String) -> OSStatus {
        writes.append("update \(token)")
        guard updateStatus == errSecSuccess else { return updateStatus }
        guard items[account] != nil else { return errSecItemNotFound }
        items[account] = token
        return errSecSuccess
    }
}

/// AppModel's handling of `.ready` and the refresh, on the pure decisions (the same calls
/// AppModel makes, without its UI state).
private struct Session {
    var generation: UInt64? = 7
    var pending: (generation: UInt64, token: String)?
    var storedAccount: String?
    var notices: [OSStatus] = []

    mutating func ready(_ gen: UInt64, userID: String, keychain: FakeKeychain) {
        let action = keychainAction(.ready(generation: gen), currentGeneration: generation, freshCapture: pending?.generation == gen)
        let token = pending?.token
        pending = nil
        if action == .write, let token {
            keychain.save(token: token, account: userID)
            storedAccount = userID
        }
    }

    mutating func refreshed(_ gen: UInt64, userID: String, token: String, keychain: FakeKeychain) {
        let action = tokenRefreshAction(
            generation: gen, userID: userID, currentGeneration: generation,
            pendingGeneration: pending?.generation, storedAccount: storedAccount
        )
        if let status = applyTokenRefresh(action, token: token, pending: &pending, keychain: keychain) {
            notices.append(status)
        }
    }
}

@Test func freshCaptureRefreshedAfterReadyEndsWithTheRefreshedToken() {
    // Core emits Ready first: the captured token is written, then updated in place.
    let keychain = FakeKeychain()
    var s = Session(pending: (7, "captured"))
    s.ready(7, userID: "1", keychain: keychain)
    s.refreshed(7, userID: "1", token: "refreshed", keychain: keychain)
    #expect(keychain.items == ["1": "refreshed"])
    #expect(keychain.writes == ["save captured", "update refreshed"])
    #expect(s.notices.isEmpty)
}

@Test func freshCaptureRefreshedBeforeReadyWritesOnlyTheRefreshedToken() {
    let keychain = FakeKeychain()
    var s = Session(pending: (7, "captured"))
    s.refreshed(7, userID: "1", token: "refreshed", keychain: keychain)
    s.ready(7, userID: "1", keychain: keychain)
    #expect(keychain.items == ["1": "refreshed"])
    #expect(keychain.writes == ["save refreshed"])
}

@Test func savedSignInIsUpdatedInPlace() {
    let keychain = FakeKeychain()
    keychain.items = ["1": "old"]
    var s = Session(storedAccount: "1")
    s.ready(7, userID: "1", keychain: keychain)
    s.refreshed(7, userID: "1", token: "new", keychain: keychain)
    #expect(keychain.items == ["1": "new"])
    #expect(keychain.writes == ["update new"])
}

@Test func staleGenerationWritesNothing() {
    let keychain = FakeKeychain()
    keychain.items = ["1": "old"]
    var s = Session(generation: 8, storedAccount: "1")
    s.refreshed(7, userID: "1", token: "new", keychain: keychain)
    #expect(keychain.writes.isEmpty)
    #expect(keychain.items == ["1": "old"])
}

@Test func afterSignOutWritesNothing() {
    // Sign Out clears the generation (and deletes the item); a late refresh must not re-create it.
    let keychain = FakeKeychain()
    var s = Session(generation: nil, storedAccount: nil)
    s.refreshed(7, userID: "1", token: "new", keychain: keychain)
    #expect(keychain.writes.isEmpty)
    #expect(tokenRefreshAction(generation: 7, userID: "1", currentGeneration: nil, pendingGeneration: nil, storedAccount: "1") == .none)
}

@Test func anotherAccountWritesNothing() {
    let keychain = FakeKeychain()
    keychain.items = ["1": "main"]
    var s = Session(storedAccount: "1")
    s.refreshed(7, userID: "2", token: "alt", keychain: keychain)
    #expect(keychain.writes.isEmpty)
    #expect(keychain.items == ["1": "main"])
}

@Test func nothingSavedWritesNothing() {
    // Session-only sign-in (the save failed): there is no item to update.
    let keychain = FakeKeychain()
    var s = Session(storedAccount: nil)
    s.refreshed(7, userID: "1", token: "new", keychain: keychain)
    #expect(keychain.writes.isEmpty)
}

@Test func updateFailureKeepsTheOldItemAndReportsTheStatus() {
    let keychain = FakeKeychain()
    keychain.items = ["1": "old"]
    keychain.updateStatus = errSecInteractionNotAllowed
    var s = Session(storedAccount: "1")
    s.refreshed(7, userID: "1", token: "new", keychain: keychain)
    #expect(keychain.items == ["1": "old"])
    #expect(s.notices == [errSecInteractionNotAllowed])
}
