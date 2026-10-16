import Foundation
import Security
import VelellaLogin

/// The saved sign-in: one generic password in the file-based login keychain. Service is fixed in
/// every build (so `swift run`, a debug binary and a bundle all share it); account = Discord user
/// id; data = the token as UTF-8. No kSecAttrAccessible / kSecAttrSynchronizable /
/// data-protection keychain (deferred to the signing milestone).
///
/// Only AppModel calls this, only on the main actor, and no automated test may: the item it
/// touches is the user's real one.
enum Keychain {
    static let service = "com.nyanako.velella"

    struct Saved {
        let token: String
        let account: String
    }

    /// One item for the service, with its account. Status is returned as-is; the caller decides.
    static func load() -> (Saved?, OSStatus) {
        let query: [CFString: Any] = [
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: service,
            kSecMatchLimit: kSecMatchLimitOne,
            kSecReturnData: true,
            kSecReturnAttributes: true,
        ]
        var result: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        guard status == errSecSuccess else { return (nil, status) }
        guard let item = result as? [CFString: Any], let data = item[kSecValueData] as? Data,
              let token = String(data: data, encoding: .utf8)
        else { return (nil, errSecDecode) }
        return (Saved(token: token, account: item[kSecAttrAccount] as? String ?? ""), status)
    }

    /// Delete-then-add. Any failure is returned; the token is never written anywhere else.
    static func save(token: String, account: String) -> OSStatus {
        let deleted = delete()
        guard deleted == errSecSuccess else { return deleted }
        let item: [CFString: Any] = [
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: service,
            kSecAttrAccount: account,
            kSecValueData: Data(token.utf8),
        ]
        return SecItemAdd(item as CFDictionary, nil)
    }

    /// A refreshed token (menu plan ID, T-8): `SecItemUpdate` of the item matching the service and
    /// `account`, so an item of another account is never touched and nothing is created.
    static func update(token: String, account: String) -> OSStatus {
        let query: [CFString: Any] = [
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: service,
            kSecAttrAccount: account,
        ]
        let change: [CFString: Any] = [kSecValueData: Data(token.utf8)]
        return SecItemUpdate(query as CFDictionary, change as CFDictionary)
    }

    /// The real keychain behind `applyTokenRefresh`.
    struct Updater: SignInUpdating {
        func update(token: String, account: String) -> OSStatus {
            Keychain.update(token: token, account: account)
        }
    }

    /// Removes every item of the service (M1 keeps one account). Not found counts as success.
    static func delete() -> OSStatus {
        let query: [CFString: Any] = [kSecClass: kSecClassGenericPassword, kSecAttrService: service]
        let status = SecItemDelete(query as CFDictionary)
        return status == errSecItemNotFound ? errSecSuccess : status
    }
}
