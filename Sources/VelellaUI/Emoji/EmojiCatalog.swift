import Foundation

/// Discord's picker categories, in the picker's order (flags last).
public enum EmojiCategory: Int, CaseIterable, Sendable {
    case people, nature, food, activity, travel, objects, symbols, flags
}

/// One emoji of the bundled catalog (emoji-toolkit 11.0.0 filtered to Emoji 16, Discord's names).
public struct EmojiEntry: Sendable, Hashable {
    /// The fully qualified characters, e.g. "👍🏻" or "❤️".
    public let sequence: String
    /// Discord's names, primary first, without colons ("thumbsup", "+1", ...).
    public let names: [String]
    /// nil for skin-tone children and the five skin-tone modifiers (they are not listed in the picker).
    public let category: EmojiCategory?
    /// Index among the category's base emoji, or among the parent's tone children.
    public let position: Int
    public let keywords: [String]
    /// Skin tones of a child, 1 (U+1F3FB) to 5 (U+1F3FF); two values for two-person sequences. Empty otherwise.
    public let tones: [Int]
    /// The base emoji of a skin-tone child.
    public let parent: String?

    public var name: String { names[0] }
    public var imageName: String { TwemojiFile.name(forSequence: sequence) }
    /// Discord draws these as text, not as an image (its `getURL` returns "").
    public var isTextOnly: Bool { EmojiCatalog.textOnly.contains(EmojiCatalog.key(sequence)) }
}

/// Twemoji's file naming rule.
public enum TwemojiFile {
    /// Lowercase hex, no padding, joined by "-"; FE0F is dropped unless the sequence has a ZWJ.
    /// "❤️" -> "2764", "4️⃣" -> "34-20e3", "❤️‍🔥" -> "2764-fe0f-200d-1f525".
    public static func name(forSequence s: String) -> String {
        var cps = s.unicodeScalars.map(\.value)
        if !cps.contains(0x200D) { cps.removeAll { $0 == 0xFE0F } }
        let name = cps.map { String($0, radix: 16) }.joined(separator: "-")
        // The one file that breaks the rule: eye in speech bubble has no FE0F at all.
        return name == "1f441-fe0f-200d-1f5e8-fe0f" ? "1f441-200d-1f5e8" : name
    }
}

public struct EmojiMatch: Sendable {
    public let entry: EmojiEntry
    public let range: Range<String.Index>
}

/// Lookups over the bundled catalog. Immutable after init.
public final class EmojiCatalog: Sendable {
    public static let shared: EmojiCatalog = {
        guard let url = Bundle.module.url(forResource: "catalog", withExtension: "json", subdirectory: "Emoji"),
            let data = try? Data(contentsOf: url), let c = try? EmojiCatalog(data: data)
        else { fatalError("emoji catalog missing from the VelellaUI bundle") }
        return c
    }()

    public let entries: [EmojiEntry]
    private let byKey: [String: Int]
    private let byName: [String: Int]
    private let byCategory: [[Int]]
    private let childrenOf: [String: [Int]]
    private let maxKeyScalars: Int

    /// ™ © ® (any presentation) stay text in Discord.
    static let textOnly: Set<String> = ["™", "©", "®"]

    /// The lookup key of a sequence: its characters without FE0F (presentation selector).
    static func key(_ s: String) -> String {
        var v = String.UnicodeScalarView()
        for u in s.unicodeScalars where u.value != 0xFE0F { v.append(u) }
        return String(v)
    }

    private struct Doc: Decodable {
        let categories: [String]
        let entries: [[Row]]
    }
    // Rows are heterogeneous JSON arrays: [cp, names, cat, pos, keywords, parent, tones].
    private enum Row: Decodable {
        case s(String), i(Int), ss([String]), ii([Int])
        init(from d: Decoder) throws {
            let c = try d.singleValueContainer()
            if let v = try? c.decode(String.self) { self = .s(v) }
            else if let v = try? c.decode(Int.self) { self = .i(v) }
            else if let v = try? c.decode([String].self) { self = .ss(v) }
            else { self = .ii(try c.decode([Int].self)) }
        }
    }

    /// `data` is catalog.json as written by scripts/emoji_catalog.py.
    public init(data: Data) throws {
        let doc = try JSONDecoder().decode(Doc.self, from: data)
        func str(_ r: Row) -> String { if case .s(let v) = r { v } else { "" } }
        func int(_ r: Row) -> Int { if case .i(let v) = r { v } else { 0 } }
        func strs(_ r: Row) -> [String] { if case .ss(let v) = r { v } else { [] } }
        func ints(_ r: Row) -> [Int] { if case .ii(let v) = r { v } else { [] } }
        func chars(_ cp: String) -> String {
            String(String.UnicodeScalarView(cp.split(separator: "-").compactMap { UInt32($0, radix: 16).flatMap(Unicode.Scalar.init) }))
        }
        var list: [EmojiEntry] = []
        for r in doc.entries {
            // tones: an empty array decodes as [String] first, which is fine, ints() then sees nothing
            let tones = ints(r[6])
            let ci = int(r[2])
            list.append(EmojiEntry(
                sequence: chars(str(r[0])), names: strs(r[1]), category: EmojiCategory(rawValue: ci),
                position: int(r[3]), keywords: strs(r[4]),
                tones: tones, parent: str(r[5]).isEmpty ? nil : chars(str(r[5]))))
        }
        entries = list
        var keys: [String: Int] = [:], names: [String: Int] = [:]
        var cats = [[Int]](repeating: [], count: EmojiCategory.allCases.count)
        var kids: [String: [Int]] = [:]
        var maxLen = 1
        for (i, e) in list.enumerated() {
            let k = Self.key(e.sequence)
            keys[k] = i
            maxLen = max(maxLen, k.unicodeScalars.count)
            for n in e.names { names[n] = i }
            if let c = e.category { cats[c.rawValue].append(i) }
            if let p = e.parent { kids[p, default: []].append(i) }
        }
        for c in 0..<cats.count { cats[c].sort { list[$0].position < list[$1].position } }
        for (p, v) in kids { kids[p] = v.sorted { list[$0].position < list[$1].position } }
        byKey = keys; byName = names; byCategory = cats; childrenOf = kids; maxKeyScalars = maxLen
    }

    /// The entry for exactly this sequence (FE0F is ignored, so "❤" and "❤️" are the same entry).
    public func entry(forSequence s: String) -> EmojiEntry? {
        byKey[Self.key(s)].map { entries[$0] }
    }

    /// The longest catalog sequence starting at `index`, so a ZWJ family or a keycap resolves as one
    /// entry. The range includes a trailing FE0F. nil when text at `index` does not start an emoji.
    public func match(in s: String, at index: String.Index) -> EmojiMatch? {
        let u = s.unicodeScalars
        guard index < u.endIndex else { return nil }
        // Plain ASCII text is the hot path of a caller scanning a message; only keycap bases (# * 0-9) can start an emoji.
        let first = u[index].value
        if first < 0xA9, !(first == 0x23 || first == 0x2A || (0x30...0x39).contains(first)) { return nil }
        // the next `maxKeyScalars` scalars that are not FE0F, with the index after each
        var ends: [String.Index] = []
        var key = String.UnicodeScalarView()
        var keys: [String] = []
        var i = index
        while i < u.endIndex, ends.count < maxKeyScalars {
            let v = u[i]
            u.formIndex(after: &i)
            if v.value == 0xFE0F { continue }
            key.append(v)
            keys.append(String(key))
            ends.append(i)
        }
        for n in stride(from: keys.count, through: 1, by: -1) {
            guard let hit = byKey[keys[n - 1]] else { continue }
            var end = ends[n - 1]
            if end < u.endIndex, u[end].value == 0xFE0F { u.formIndex(after: &end) }
            return EmojiMatch(entry: entries[hit], range: index..<end)
        }
        return nil
    }

    /// By Discord name or alias, without colons ("thumbsup", "+1", "skin-tone-2").
    public func entry(named name: String) -> EmojiEntry? {
        byName[name].map { entries[$0] }
    }

    /// The category's base emoji in Discord's order.
    public func entries(in category: EmojiCategory) -> [EmojiEntry] {
        byCategory[category.rawValue].map { entries[$0] }
    }

    /// Skin-tone children of a base emoji in tone order (5, or 25 for two-person emoji).
    public func variants(of base: EmojiEntry) -> [EmojiEntry] {
        (childrenOf[base.sequence] ?? []).map { entries[$0] }
    }

    /// The child for `tones` (1...5; one value, or two for two-person emoji).
    public func variant(of base: EmojiEntry, tones: [Int]) -> EmojiEntry? {
        variants(of: base).first { $0.tones == tones }
    }

    public func variant(of base: EmojiEntry, tone: Int) -> EmojiEntry? {
        variant(of: base, tones: [tone])
    }

    /// Keyword search over the base emoji: exact name, then name prefix, then name contains, then keyword
    /// contains; each tier in picker order. Names match through every alias. `query` may carry colons.
    /// UNVERIFIED against the official picker: the bundle does not show its ranking (emoji-research.md
    /// has no search section), so the tiers are the contract's prefix-then-contains plus an exact tier
    /// and a keyword tier.
    public func search(_ query: String, limit: Int = .max) -> [EmojiEntry] {
        let q = query.lowercased().trimmingCharacters(in: CharacterSet(charactersIn: ": "))
        guard !q.isEmpty else { return [] }
        var tiers: [[EmojiEntry]] = [[], [], [], []]
        for c in EmojiCategory.allCases {
            for i in byCategory[c.rawValue] {
                let e = entries[i]
                if e.names.contains(q) { tiers[0].append(e) }
                else if e.names.contains(where: { $0.hasPrefix(q) }) { tiers[1].append(e) }
                else if e.names.contains(where: { $0.contains(q) }) { tiers[2].append(e) }
                else if e.keywords.contains(where: { $0.lowercased().contains(q) }) { tiers[3].append(e) }
            }
        }
        return Array(tiers.joined().prefix(limit))
    }
}
