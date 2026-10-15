import SwiftUI

/// The line under the composer that says who is typing (wave 3, track R). Core resolves the
/// names and drops them after 10 s; this only words and draws them. Names are payload text: they
/// show verbatim, isolated like every substituted name, never as markup.
enum TypingLabel {
    enum Part: Equatable {
        case name(String)
        case text(String)
    }

    /// Discord's wording: one, two or three names, "Several people" from four on, nothing for none.
    static func parts(_ names: [String]) -> [Part] {
        let n = names.map(MarkdownText.isolated)
        return switch n.count {
        case 0: []
        case 1: [.name(n[0]), .text(" is typing…")]
        case 2: [.name(n[0]), .text(" and "), .name(n[1]), .text(" are typing…")]
        case 3: [.name(n[0]), .text(", "), .name(n[1]), .text(" and "), .name(n[2]), .text(" are typing…")]
        default: [.text("Several people are typing…")]
        }
    }

    static func string(_ names: [String]) -> String {
        parts(names).map { switch $0 { case .name(let s), .text(let s): s } }.joined()
    }

    /// Names semibold, the rest regular; one verbatim `AttributedString`, so no part is parsed.
    static func attributed(_ names: [String]) -> AttributedString {
        var out = AttributedString()
        for part in parts(names) {
            switch part {
            case .name(let s):
                var run = AttributedString(s)
                run.font = TypingMetrics.nameFont
                out.append(run)
            case .text(let s):
                out.append(AttributedString(s))
            }
        }
        return out
    }
}

/// Discord's typing indicator, measured from its client: 7 pt dots with a 2 pt gap, a 1.2 s pulse.
enum TypingMetrics {
    static let dotSize: CGFloat = 7
    static let dotGap: CGFloat = 2
    static let pulse: TimeInterval = 1.2
    /// Each dot trails the one before by a quarter of the pulse, so the three read as a wave.
    static let dotLag: TimeInterval = pulse / 4
    static let minOpacity = 0.3
    /// Between the dots and the words.
    static let dotsToText: CGFloat = 6
    /// The line's reserved height: the row never changes the composer's height as names come and go.
    static let height: CGFloat = 16
    static let font = Font.system(size: 12)
    static let nameFont = Font.system(size: 12, weight: .semibold)

    /// Opacity of dot `index` at `time`: a sine pulse from `minOpacity` to 1, period `pulse`.
    static func opacity(at time: TimeInterval, dot index: Int) -> Double {
        let phase = (time - Double(index) * dotLag) / pulse
        return minOpacity + (1 - minOpacity) * (1 + sin(2 * .pi * phase)) / 2
    }
}

struct TypingIndicator: View {
    let names: [String]
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        HStack(spacing: TypingMetrics.dotsToText) {
            if !names.isEmpty {
                dots
                Text(TypingLabel.attributed(names))
                    .font(TypingMetrics.font)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                    .accessibilityLabel(Text(verbatim: TypingLabel.string(names)))
            }
            Spacer(minLength: 0)
        }
        .frame(height: TypingMetrics.height)
    }

    private var dots: some View {
        TimelineView(.animation(paused: reduceMotion)) { timeline in
            let time = timeline.date.timeIntervalSinceReferenceDate
            HStack(spacing: TypingMetrics.dotGap) {
                ForEach(0..<3, id: \.self) { index in
                    Circle()
                        .fill(.secondary)
                        .frame(width: TypingMetrics.dotSize, height: TypingMetrics.dotSize)
                        .opacity(reduceMotion ? 0.6 : TypingMetrics.opacity(at: time, dot: index))
                }
            }
        }
        .accessibilityHidden(true)
    }
}
