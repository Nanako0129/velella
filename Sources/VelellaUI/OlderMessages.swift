import SwiftUI

private typealias T = DesignTokens

/// Older-history state of the open conversation's messages (wave 3 P). `failed` carries fixed text.
public enum OlderState: Hashable, Sendable {
    case idle, loading, reachedStart
    case failed(String)
}

/// The top row of the message list. It always has the same height, so the spinner (or the failure
/// text and its Retry button) appearing and going never moves the rows below it.
struct OlderRow: View {
    /// Room for the spinner, or one line of failure text with its button.
    static let height: CGFloat = 32

    let older: OlderState
    var onRetry: (() -> Void)?

    var body: some View {
        Group {
            switch older {
            case .loading:
                ProgressView().controlSize(.small)
            case .failed(let text):
                HStack(spacing: T.Spacing.sm) {
                    Label { Text(verbatim: text) } icon: { Image(systemName: "exclamationmark.triangle.fill") }
                    if let onRetry { Button("Retry", action: onRetry) }
                }
                .font(T.Typography.timestamp)
                .lineLimit(1)
                .foregroundStyle(.secondary)
            case .idle, .reachedStart:
                Color.clear
            }
        }
        .frame(maxWidth: .infinity)
        .frame(height: Self.height)
    }
}
