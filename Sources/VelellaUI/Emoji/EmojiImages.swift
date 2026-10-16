import AppKit

/// Twemoji artwork as `NSImage` (vector SVG, drawn at any size), loaded on demand into an in-memory
/// LRU cache bounded by the bytes of the SVG files it holds.
@MainActor
public final class EmojiImageProvider {
    public static let shared = EmojiImageProvider()

    public let byteLimit: Int
    private let folder: URL?
    private var cache: [String: (image: NSImage, cost: Int)] = [:]
    private var lru: [String] = []  // oldest first
    public private(set) var cachedBytes = 0
    public var cachedCount: Int { cache.count }

    /// The bundled Twemoji SVG folder.
    public nonisolated static var bundledFolder: URL? {
        Bundle.module.resourceURL?.appendingPathComponent("Twemoji", isDirectory: true)
    }

    /// `folder` overrides the bundled Twemoji folder.
    public init(byteLimit: Int = 8 << 20, folder: URL? = nil) {
        self.byteLimit = byteLimit
        self.folder = folder ?? Self.bundledFolder
    }

    /// The image for an entry, or nil when its file is missing or does not decode.
    public func image(for entry: EmojiEntry) -> NSImage? { image(named: entry.imageName) }

    /// By Twemoji file name (no extension), e.g. "2764".
    public func image(named name: String) -> NSImage? {
        if let hit = cache[name] {
            touch(name)
            return hit.image
        }
        guard let url = folder?.appendingPathComponent(name + ".svg"),
            let data = try? Data(contentsOf: url), let image = NSImage(data: data)
        else { return nil }
        cache[name] = (image, data.count)
        lru.append(name)
        cachedBytes += data.count
        while cachedBytes > byteLimit, lru.count > 1 {  // the image just loaded always stays
            let old = lru.removeFirst()
            cachedBytes -= cache.removeValue(forKey: old)?.cost ?? 0
        }
        return image
    }

    private func touch(_ name: String) {
        if let i = lru.lastIndex(of: name) { lru.remove(at: i) }
        lru.append(name)
    }
}
