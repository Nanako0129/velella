import AppKit
import SwiftUI

// The message column's own first responder (docs/plans/selection.md, N2 Drag): ⌘C reaches the
// selection through it once every text view the press was made in has been recycled.

struct SelectionResponder: NSViewRepresentable {
    let coordinator: SelectionCoordinator

    func makeNSView(context: Context) -> SelectionResponderView { SelectionResponderView() }

    func updateNSView(_ view: SelectionResponderView, context: Context) {
        view.coordinator = coordinator
        view.pasteboard = context.environment.textPasteboard.board ?? .general
        coordinator.responder = view
    }
}

final class SelectionResponderView: NSView {
    weak var coordinator: SelectionCoordinator?
    var pasteboard: NSPasteboard = .general

    override var acceptsFirstResponder: Bool { true }
    /// Never takes a press: it is only ever made first responder by the coordinator.
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
    override func isAccessibilityElement() -> Bool { false }

    override func becomeFirstResponder() -> Bool { MainActor.assumeIsolated { coordinator?.repaint() }; return true }
    override func resignFirstResponder() -> Bool { MainActor.assumeIsolated { coordinator?.repaint() }; return true }

    @objc func copy(_ sender: Any?) { MainActor.assumeIsolated { coordinator?.copy(to: pasteboard) } }

    @objc func validateUserInterfaceItem(_ item: any NSValidatedUserInterfaceItem) -> Bool {
        MainActor.assumeIsolated { item.action == #selector(copy(_:)) && coordinator?.hasSelection == true }
    }

    override func validRequestor(forSendType sendType: NSPasteboard.PasteboardType?, returnType: NSPasteboard.PasteboardType?) -> Any? { nil }
}
