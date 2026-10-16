import SwiftUI

// Swiftcord v2's tooltip (docs/plans/markdown.md, P1): a glass capsule centred just above the
// hovered source, flipped below when it does not fit, clamped inside its container. Values come
// from `DiscordUITooltips` in the v2 binary (refs/swiftcord-v2-teardown.md, "Tooltips", kept out
// of git); addresses below are virtual addresses in that binary.

enum TooltipStyle {
    /// Gap between the source and the tooltip (v2 `TooltipPresenter.spacing`: 6.0 at
    /// 0x1029f5cc8 and 0x100ba3278, the default getter at 0x102a05730).
    static let spacing: CGFloat = 6
    /// The container is inset by this on every side before the tooltip is placed (0x102a07570).
    static let containerInset: CGFloat = 8
    /// Widest tooltip (0x102a07570).
    static let maxWidth: CGFloat = 360
    /// Text 14 pt medium; padding 12 horizontal and 8 vertical (content view at 0x102a05c5c).
    static let font = Font.system(size: 14, weight: .medium)
    static let horizontalPadding: CGFloat = 12
    static let verticalPadding: CGFloat = 8
    /// Fade and move with a critically damped spring of 0.25 s response (0x102a07114); v2 skips
    /// the animation when Reduce Motion is on.
    static let animation = Animation.spring(response: 0.25, dampingFraction: 1)
    // No show delay: v2's text views show a tip at once (delay 0 at 0x100ba3278, the UIKit call
    // site behind message timestamps; the user's capture, 2026-10-16). Header and gutter times
    // match it; v2's SwiftUI modifier default was not recovered.
}

enum TooltipPlacement {
    /// Where v2 puts a tooltip of `size` for a `source` rect (all in one coordinate space), as
    /// 0x102a07570 does: sizes truncated to whole points; centred on the source and clamped to
    /// the container inset by `containerInset`; above by `spacing`, else below, else the container top.
    static func origin(size: CGSize, source: CGRect, container: CGRect) -> CGPoint {
        let area = container.insetBy(dx: TooltipStyle.containerInset, dy: TooltipStyle.containerInset)
        let width = size.width.rounded(.towardZero), height = size.height.rounded(.towardZero)
        let centred = max(source.midX - width / 2, area.minX)
        let x = min(centred, max(area.maxX - width, area.minX))
        var y = source.minY - height - TooltipStyle.spacing
        if y < area.minY { y = min(source.maxY + TooltipStyle.spacing, area.maxY - height) }
        return CGPoint(x: x, y: max(y, area.minY))
    }
}

/// The coordinate space a scrolling column names, and the container (in that space) its
/// tooltips stay inside. Without one a tooltip is not clamped.
enum TooltipSpace {
    static let name = "tooltip-container"
    static let unbounded = CGRect(x: -1e6, y: -1e6, width: 2e6, height: 2e6)
}

extension EnvironmentValues {
    @Entry var tooltipContainer: CGRect?
    /// The tooltip surface: v2's clear glass in the app. Tests use `.solid`, because `ImageRenderer`
    /// draws glass only after other views have rendered in the process (order-dependent, measured).
    @Entry var tooltipSurface: TooltipSurfaceKind = .glass
}

/// The tooltip surface: v2 draws the text on clear Liquid Glass in a capsule (the shape is a
/// generic parameter in v2, the user's capture shows a capsule).
enum TooltipSurfaceKind { case glass, solid }

struct TooltipBubble: View {
    let text: String
    @Environment(\.tooltipSurface) private var surface

    var body: some View {
        Text(verbatim: text)
            .font(TooltipStyle.font)
            // The host's text styling must not leak in: a paragraph's 22 pt line box made the
            // capsule 38 pt tall instead of v2's 33 (user's capture, 2026-10-16), and a quote's
            // secondary colour dimmed it.
            .lineHeight(nil)
            .foregroundStyle(.primary)
            .padding(.horizontal, TooltipStyle.horizontalPadding)
            .padding(.vertical, TooltipStyle.verticalPadding)
            .modifier(TooltipSurfaceModifier(kind: surface))
            .modifier(TipSize())
    }
}

/// Sizes the tip to its content the way v2 does (`sizeThatFits` at 0x102a07570): the ideal width,
/// at most `maxWidth`, wrapping only past it. `.frame(maxWidth:)` instead stretched the capsule to
/// whatever its host offered, up to 360 (the user's capture, 2026-10-16).
private struct TipSize: ViewModifier {
    func body(content: Content) -> some View { TipLayout { content } }
}

private struct TipLayout: Layout {
    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        guard let tip = subviews.first else { return .zero }
        let width = min(tip.sizeThatFits(.unspecified).width, TooltipStyle.maxWidth)
        return tip.sizeThatFits(ProposedViewSize(width: width, height: nil))
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        subviews.first?.place(at: bounds.origin, proposal: ProposedViewSize(bounds.size))
    }
}

private struct TooltipSurfaceModifier: ViewModifier {
    let kind: TooltipSurfaceKind

    func body(content: Content) -> some View {
        switch kind {
        case .glass: content.glassEffect(.clear, in: .capsule)
        case .solid: content.background(Color.gray, in: .capsule)
        }
    }
}

/// A tooltip to show: `rect` is the hovered source in the host's coordinates, nil for the host.
struct TooltipRequest: Equatable {
    var text: String
    var rect: CGRect?
}

/// Draws `request` over the host without taking layout space or hits.
private struct TooltipOverlay: ViewModifier {
    let request: TooltipRequest?
    @Environment(\.tooltipContainer) private var container
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    func body(content: Content) -> some View {
        content
            .overlay(alignment: .topLeading) {
                if let request {
                    GeometryReader { host in
                        let frame = container == nil ? .zero : host.frame(in: .named(TooltipSpace.name))
                        let area = container?.offsetBy(dx: -frame.minX, dy: -frame.minY) ?? TooltipSpace.unbounded
                        let source = request.rect ?? CGRect(origin: .zero, size: host.size)
                        Color.clear.overlay(alignment: .topLeading) {
                            TooltipBubble(text: request.text)
                                .alignmentGuide(.leading) { d in -TooltipPlacement.origin(size: d.size, source: source, container: area).x }
                                .alignmentGuide(.top) { d in -TooltipPlacement.origin(size: d.size, source: source, container: area).y }
                                .transition(.opacity)
                        }
                    }
                    .allowsHitTesting(false)
                }
            }
            .animation(reduceMotion ? nil : TooltipStyle.animation, value: request)
    }
}

private extension ViewDimensions {
    var size: CGSize { CGSize(width: width, height: height) }
}

extension View {
    /// Shows `request` as the v2 tooltip (the paragraph picks runs itself).
    func tooltipOverlay(_ request: TooltipRequest?) -> some View { modifier(TooltipOverlay(request: request)) }

    /// A v2 tooltip on the whole view while the pointer is on it.
    func tooltip(_ text: String) -> some View { modifier(HoverTooltip(text: text)) }
}

private struct HoverTooltip: ViewModifier {
    let text: String
    @State private var shown = false

    func body(content: Content) -> some View {
        content
            .onHover { shown = $0 }
            .tooltipOverlay(shown ? TooltipRequest(text: text, rect: nil) : nil)
    }
}
