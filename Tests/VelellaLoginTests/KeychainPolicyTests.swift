import Security
import Testing
@testable import VelellaLogin

// The pure decision only. No test anywhere calls the Security framework's item APIs: the service
// name is fixed in every build, so such a test would act on the user's real saved sign-in.

@Test func readyAfterSignOutIsNone() {
    // Sign Out clears the current generation; the old session's READY can still arrive.
    #expect(keychainAction(.ready(generation: 1), currentGeneration: nil, freshCapture: true) == .none)
}

@Test func readyWithStaleGenerationIsNone() {
    // A newer login (generation 2) superseded the session that produced this READY.
    #expect(keychainAction(.ready(generation: 1), currentGeneration: 2, freshCapture: true) == .none)
}

@Test func readyOfLoadedTokenIsNone() {
    #expect(keychainAction(.ready(generation: 2), currentGeneration: 2, freshCapture: false) == .none)
}

@Test func readyOfFreshCaptureWrites() {
    #expect(keychainAction(.ready(generation: 2), currentGeneration: 2, freshCapture: true) == .write)
}

@Test func loadErrorOtherThanNotFoundIsNoneNeverDelete() {
    for status in [errSecInteractionNotAllowed, errSecAuthFailed, errSecIO, errSecDecode, errSecNotAvailable] {
        #expect(keychainAction(.loaded(status: status), currentGeneration: nil, freshCapture: false) == .none)
    }
    #expect(keychainAction(.loaded(status: errSecItemNotFound), currentGeneration: nil, freshCapture: false) == .none)
}

@Test func saveFailureIsSessionOnly() {
    for status in [errSecDuplicateItem, errSecInteractionNotAllowed, errSecIO, errSecWrPerm] {
        #expect(keychainAction(.saved(status: status), currentGeneration: 2, freshCapture: true) == .sessionOnly)
    }
    #expect(keychainAction(.saved(status: errSecSuccess), currentGeneration: 2, freshCapture: true) == .none)
}

@Test func authFailedCurrentGenerationDeletes() {
    #expect(keychainAction(.authFailed(generation: 2), currentGeneration: 2, freshCapture: false) == .delete)
}

@Test func authFailedStaleGenerationIsNone() {
    #expect(keychainAction(.authFailed(generation: 1), currentGeneration: 2, freshCapture: false) == .none)
    #expect(keychainAction(.authFailed(generation: 1), currentGeneration: nil, freshCapture: false) == .none)
}

@Test func signOutDeletes() {
    #expect(keychainAction(.signedOut, currentGeneration: nil, freshCapture: false) == .delete)
}
