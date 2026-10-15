import AppKit
import ImageIO
import SwiftUI

// The one loader for embed images, embed icons and image attachments (wave 3 E, Image loading
// row; closes M2 F11 for attachments). Core sends only `media_url` URLs; this re-checks each one
// before any fetch and each redirect target, sends no cookies, credentials or added headers,
// refuses bodies over 16 MiB and images over 50 MP, and decodes a thumbnail no larger than twice
// the display box. Results stay in memory (an `NSCache` with a byte cost limit).

/// Box sizes for media (wave 3 E, Display row). Provisional maxima, to be checked against a capture.
enum MediaBox {
    /// Embed images and image attachments fit inside this box.
    static let image = CGSize(width: 400, height: 300)
    /// Embed thumbnails fit inside this box.
    static let thumbnail = CGSize(width: 80, height: 80)
    /// Fixed icon sides: author 24, footer 20 (Discord's `.embedAuthorIcon`, `.embedFooterIcon`).
    static let authorIcon = CGSize(width: 24, height: 24)
    static let footerIcon = CGSize(width: 20, height: 20)

    /// (`width`, `height`) scaled down to fit inside `max`, keeping the ratio, never scaled up.
    /// In `Double` throughout; a non-positive or non-finite side gives an empty box.
    static func fit(width: Double, height: Double, in max: CGSize) -> CGSize {
        guard width.isFinite, height.isFinite, width > 0, height > 0 else { return .zero }
        let scale = Swift.min(1, Double(max.width) / width, Double(max.height) / height)
        return CGSize(width: width * scale, height: height * scale)
    }
}

final class MediaImages: @unchecked Sendable {
    // @unchecked: `NSCache` is thread-safe; every other stored property is immutable and Sendable.

    static let shared = MediaImages()

    /// Mirrors core's `media::MEDIA_HOSTS`: Discord's media proxy and CDN hosts.
    static let hosts: Set<String> = [
        "media.discordapp.net",
        "images-ext-1.discordapp.net",
        "images-ext-2.discordapp.net",
        "cdn.discordapp.com",
    ]
    /// The hosts that resize on request (`width`/`height`); provisional, confirmed on a live payload.
    static let resizingHosts: Set<String> = ["media.discordapp.net", "images-ext-1.discordapp.net", "images-ext-2.discordapp.net"]
    /// A body above this is refused, by its declared length or while it streams.
    static let maxBytes = 16 << 20
    /// An image declaring more pixels than this is never decoded.
    static let maxPixels = 50_000_000
    /// Decoded images kept, by byte cost (4 bytes a pixel).
    static let cacheBytes = 64 << 20

    private let session: URLSession
    /// Test seam: maps a checked Discord URL to the local test server. nil in the app.
    private let rewrite: (@Sendable (URL) -> URL)?
    private let cache = NSCache<NSURL, CGImage>()

    init(rewrite: (@Sendable (URL) -> URL)? = nil) {
        let config = URLSessionConfiguration.ephemeral
        config.httpCookieStorage = nil
        config.httpShouldSetCookies = false
        config.httpCookieAcceptPolicy = .never
        config.urlCredentialStorage = nil
        config.urlCache = nil
        session = URLSession(configuration: config)
        self.rewrite = rewrite
        cache.totalCostLimit = Self.cacheBytes
    }

    /// The Swift side of core's `media_url`: `https`, a host in `hosts`, no port, no user or
    /// password. Core has already canonicalised the URL (lowercase host, default port dropped).
    static func fetchable(_ url: URL) -> Bool {
        url.scheme?.lowercased() == "https"
            && url.host().map { hosts.contains($0.lowercased()) } == true
            && url.port == nil && url.user == nil && url.password == nil
    }

    /// What is requested for a `box` (points) at `scale`: a resizing host is asked for the display
    /// size in pixels (provisional parameters); any other host gets the URL as it is.
    static func request(_ url: URL, box: CGSize, scale: CGFloat) -> URL {
        guard let host = url.host()?.lowercased(), resizingHosts.contains(host),
              var parts = URLComponents(url: url, resolvingAgainstBaseURL: false) else { return url }
        func pixels(_ side: CGFloat) -> String {
            let p = side * scale
            return String(p.isFinite ? Int(Swift.min(Swift.max(p.rounded(.up), 1), 16384)) : 1)
        }
        parts.queryItems = (parts.queryItems ?? []).filter { $0.name != "width" && $0.name != "height" }
            + [URLQueryItem(name: "width", value: pixels(box.width)), URLQueryItem(name: "height", value: pixels(box.height))]
        return parts.url ?? url
    }

    /// The accessibility label of an image: its alt text verbatim, else "Image"; never a URL.
    static func label(_ alt: String?) -> String {
        guard let alt, !alt.isEmpty else { return "Image" }
        return alt
    }

    /// The image at `url` for a display `box` (points) at `scale`, or nil (not fetchable, failed,
    /// too large, cancelled). Off the main actor; cancelled with the calling task.
    @concurrent
    func image(_ url: URL, box: CGSize, scale: CGFloat) async -> CGImage? {
        guard Self.fetchable(url) else { return nil }
        let requested = Self.request(url, box: box, scale: scale)
        if let hit = cache.object(forKey: requested as NSURL) { return hit }
        guard let data = await fetch(requested),
              let image = Self.decode(data, maxSide: 2 * Swift.max(box.width, box.height)) else { return nil }
        cache.setObject(image, forKey: requested as NSURL, cost: image.width * image.height * 4)
        return image
    }

    /// The body of a 200 response, at most `maxBytes`; redirects are followed only to fetchable
    /// targets.
    func fetch(_ url: URL) async -> Data? {
        let target = rewrite?(url) ?? url
        guard let (bytes, response) = try? await session.bytes(for: URLRequest(url: target), delegate: RedirectCheck(rewrite: rewrite)) else {
            return nil
        }
        guard (response as? HTTPURLResponse)?.statusCode == 200, response.expectedContentLength <= Int64(Self.maxBytes) else {
            bytes.task.cancel()
            return nil
        }
        var data = Data()
        do {
            for try await byte in bytes {
                data.append(byte)
                if data.count > Self.maxBytes {
                    bytes.task.cancel()
                    return nil
                }
            }
        } catch {
            return nil
        }
        return data
    }

    /// A thumbnail of at most `maxSide` pixels on its longest side, only when the header shows at
    /// most `maxPixels` pixels.
    static func decode(_ data: Data, maxSide: CGFloat) -> CGImage? {
        guard maxSide.isFinite, maxSide >= 1,
              let source = CGImageSourceCreateWithData(data as CFData, [kCGImageSourceShouldCache: false] as CFDictionary),
              let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
              let width = properties[kCGImagePropertyPixelWidth] as? Int,
              let height = properties[kCGImagePropertyPixelHeight] as? Int,
              width > 0, height > 0 else { return nil }
        let (pixels, overflow) = width.multipliedReportingOverflow(by: height)
        guard !overflow, pixels <= maxPixels else { return nil }
        let options: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceShouldCacheImmediately: true,
            kCGImageSourceThumbnailMaxPixelSize: Int(maxSide.rounded(.up)),
        ]
        return CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary)
    }
}

/// Follows a redirect only to a fetchable target (through the test seam); otherwise cancels.
private final class RedirectCheck: NSObject, URLSessionTaskDelegate, Sendable {
    let rewrite: (@Sendable (URL) -> URL)?
    init(rewrite: (@Sendable (URL) -> URL)?) { self.rewrite = rewrite }

    func urlSession(
        _ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest
    ) async -> URLRequest? {
        guard let url = request.url, MediaImages.fetchable(url) else {
            task.cancel()
            return nil
        }
        var next = request
        next.url = rewrite?(url) ?? url
        return next
    }
}

/// One media image in a fixed box, loaded through `MediaImages` while on screen (the load is
/// cancelled when the view goes away). Accessibility: the alt text or "Image".
struct MediaImageView: View {
    let url: URL
    let box: CGSize
    var alt: String?
    var loader: MediaImages = .shared
    @Environment(\.displayScale) private var scale
    @State private var image: CGImage?

    var body: some View {
        ZStack {
            if let image {
                Image(decorative: image, scale: 1).resizable()
            } else {
                MediaPlaceholder()
            }
        }
        .frame(width: box.width, height: box.height)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(Text(verbatim: MediaImages.label(alt)))
        .accessibilityAddTraits(.isImage)
        .task(id: url) {
            image = nil
            image = await loader.image(url, box: box, scale: scale)
        }
    }
}

/// What a box shows before its image arrives, or instead of it.
struct MediaPlaceholder: View {
    var body: some View { DesignTokens.Palette.replyBarFill }
}

/// Media under a spoiler: a cover of the same box, with nothing inside it built (so nothing is
/// fetched) until a click reveals it. Accessibility reads only "Spoiler".
struct SpoilerMediaCover: View {
    let box: CGSize
    let reveal: () -> Void

    var body: some View {
        RoundedRectangle(cornerRadius: DesignTokens.Radius.messageImage, style: .continuous)
            .fill(MarkdownPalette.spoilerCover)
            .overlay { Text("SPOILER").font(.caption.weight(.bold)).foregroundStyle(.white) }
            .frame(width: box.width, height: box.height)
            .contentShape(Rectangle())
            .onTapGesture(perform: reveal)
            .accessibilityElement(children: .ignore)
            .accessibilityLabel(Text(verbatim: "Spoiler"))
            .accessibilityAddTraits(.isButton)
    }
}
