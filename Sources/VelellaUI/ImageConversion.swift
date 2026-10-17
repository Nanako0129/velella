import CryptoKit
import Foundation
import ImageIO
import UniformTypeIdentifiers

/// HEIC and JXR to JPEG on add (composer plan D16a, CUA "HEIC / JXR conversion"; OA A45-A51). The
/// official converter is not on disk (A51), so the parts it leaves open are decided here: ImageIO
/// decodes, the EXIF orientation is applied to the pixels, every piece of metadata is dropped (what a
/// browser canvas gives), and the JPEG is encoded at the experiment's quality / 100. A result over the
/// experiment's size, or any failure, means the original is uploaded: the caller keeps it, silently.
enum ImageConversion {
    /// A46: the MIME type decides, and only without one the extension does.
    static func kind(contentType: String, filename: String) -> ConvertibleKind? {
        switch contentType.lowercased() {
        case "image/heic", "image/heif", "image/heic-sequence", "image/heif-sequence": return .heic
        case "image/jxr", "image/vnd.ms-photo": return .jxr
        default: break
        }
        switch (filename as NSString).pathExtension.lowercased() {
        case "heic", "heif", "hif": return .heic
        case "jxr", "wdp", "hdp": return .jxr
        default: return nil
        }
    }

    /// A46's fallback for an empty MIME type; the descriptor keeps the original's MIME (A48).
    static func originalContentType(_ contentType: String, filename: String, kind: ConvertibleKind) -> String {
        guard contentType.isEmpty else { return contentType }
        switch kind {
        case .jxr: return "image/jxr"
        case .heic: return (filename as NSString).pathExtension.lowercased() == "heic" ? "image/heic" : "image/heif"
        }
    }

    /// The bundle's rewrite helper (A51): `.heic/.heif/.hif` and `.jxr/.wdp/.hdp` become `.jpg`, any
    /// other name gets `.jpg` appended.
    static func rewrittenName(_ name: String) -> String {
        let ns = name as NSString
        switch ns.pathExtension.lowercased() {
        case "heic", "heif", "hif", "jxr", "wdp", "hdp": return ns.deletingPathExtension + ".jpg"
        default: return name + ".jpg"
        }
    }

    struct Converted {
        var jpeg: Data
        /// Of the original's bytes, 32 lowercase hex digits (A49).
        var originalMD5: String
    }

    /// nil on any failure, or when the JPEG is larger than `maxFileSizeBytes`.
    static func convert(_ file: URL, quality: UInt8, maxFileSizeBytes: UInt64) -> Converted? {
        guard let md5 = md5Hex(of: file), let source = CGImageSourceCreateWithURL(file as CFURL, nil),
              CGImageSourceGetCount(source) > 0,
              let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
              let width = properties[kCGImagePropertyPixelWidth] as? Int, let height = properties[kCGImagePropertyPixelHeight] as? Int
        else { return nil }
        // The thumbnail path is how ImageIO applies the orientation to the pixels; asked for the full
        // size it is the decoded image, upright, with no metadata attached.
        let options: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceThumbnailMaxPixelSize: max(width, height),
        ]
        guard let image = CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary) else { return nil }
        let data = NSMutableData()
        guard let destination = CGImageDestinationCreateWithData(data, UTType.jpeg.identifier as CFString, 1, nil) else { return nil }
        CGImageDestinationAddImage(destination, image, [
            kCGImageDestinationLossyCompressionQuality: Double(min(quality, 100)) / 100,
            // A JPEG has no alpha: what was transparent lands on white.
            kCGImageDestinationBackgroundColor: CGColor(gray: 1, alpha: 1),
        ] as CFDictionary)
        guard CGImageDestinationFinalize(destination), UInt64(data.length) <= maxFileSizeBytes, data.length > 0 else { return nil }
        return Converted(jpeg: data as Data, originalMD5: md5)
    }

    /// MD5 of a file's bytes, read in pieces.
    static func md5Hex(of file: URL) -> String? {
        guard let handle = try? FileHandle(forReadingFrom: file) else { return nil }
        defer { try? handle.close() }
        var md5 = Insecure.MD5()
        while let piece = try? handle.read(upToCount: 1 << 20), !piece.isEmpty { md5.update(data: piece) }
        return md5.finalize().map { String(format: "%02x", $0) }.joined()
    }
}
