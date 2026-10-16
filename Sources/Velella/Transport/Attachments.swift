import Foundation
import VelellaFFI

// Attached files on the app side (composer plan CU-1, CU-6, CU-7): the attach-time check that
// records a file's identity, the temporary files the composer synthesizes, and the redaction of
// the FFI objects that carry a path, filename or alt text.

public enum AttachmentFiles {
    /// CU-6, at attach time: the path with its links resolved must be a regular file (no
    /// directory or package, FIFO, device or socket); its (device, inode, size, mtime) are
    /// recorded so the upload can refuse a file swapped in later. nil: not attachable.
    public static func identity(of url: URL) -> UploadFile? {
        guard url.isFileURL else { return nil }
        let path = url.resolvingSymlinksInPath().path
        var st = stat()
        guard lstat(path, &st) == 0, let id = FileIdentity(st) else { return nil }
        return UploadFile(path: path, device: id.device, inode: id.inode, size: id.size, mtimeNs: id.mtimeNs)
    }

    /// Where synthesized files live: one directory per file under `velella-attachments/`.
    public static var root: URL {
        FileManager.default.temporaryDirectory.appendingPathComponent("velella-attachments", isDirectory: true)
    }

    /// CU-1: writes `data` as `<root>/<UUID>/<name>` without overwriting anything, so two pastes
    /// in two conversations never share a file. `name` must be one path component.
    public static func write(_ data: Data, named name: String) throws -> URL {
        guard !name.isEmpty, name != ".", name != "..", !name.contains("/"), !name.contains("\0") else {
            throw CocoaError(.fileWriteInvalidFileName)
        }
        let directory = root.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let file = directory.appendingPathComponent(name, isDirectory: false)
        try data.write(to: file, options: .withoutOverwriting)
        return file
    }

    /// Deletes a synthesized file with its directory (send success, removal, discard, teardown);
    /// anything outside `<root>/<UUID>/` is left alone.
    public static func remove(_ file: URL) {
        let directory = file.standardizedFileURL.deletingLastPathComponent()
        guard directory.deletingLastPathComponent().path == root.standardizedFileURL.path,
              UUID(uuidString: directory.lastPathComponent) != nil else { return }
        try? FileManager.default.removeItem(at: directory)
    }

    /// The launch sweep: every synthesized file a previous run left.
    public static func sweep() {
        try? FileManager.default.removeItem(at: root)
    }
}

// MARK: Redaction (CU-7)

extension UploadFile: CustomReflectable {
    public var customMirror: Mirror { Mirror(self, children: [], displayStyle: .class) }
}

extension AttachmentDescriptor: CustomReflectable {
    public var customMirror: Mirror { Mirror(self, children: [], displayStyle: .class) }
}
