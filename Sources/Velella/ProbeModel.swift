import Foundation
import VelellaFFI

/// Bridges the Rust callback (arbitrary thread) onto the MainActor through an AsyncStream,
/// after element-x-ios's SDKListener. When Rust drops the listener (subscription handle
/// dropped), deinit finishes the stream so the consumer task ends.
private final class TickBridge: TickListener, Sendable {
    let stream: AsyncStream<UInt64>
    private let continuation: AsyncStream<UInt64>.Continuation

    init() {
        (stream, continuation) = AsyncStream<UInt64>.makeStream()
    }

    func onTick(count: UInt64) {
        continuation.yield(count)
    }

    deinit { continuation.finish() }
}

@MainActor
@Observable
final class ProbeModel {
    var probe = "…"
    var ticks: UInt64 = 0
    /// Holding this keeps the Rust tick task alive; setting it to nil runs Rust `Drop`.
    private var subscription: Subscription?

    func start() async {
        probe = await runtimeProbe()
        let bridge = TickBridge()
        let stream = bridge.stream
        subscription = subscribeTicks(listener: bridge)
        for await count in stream {
            ticks = count
        }
    }

    func stop() {
        subscription = nil
    }

    var isRunning: Bool { subscription != nil }
}
