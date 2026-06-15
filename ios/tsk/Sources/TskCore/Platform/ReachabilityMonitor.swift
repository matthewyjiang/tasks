import Combine
import Foundation
import Network

public enum ReachabilityStatus: Equatable, Sendable {
    case online
    case offline
    case requiresConnection

    public var isOnline: Bool {
        self == .online
    }

    public var label: String {
        switch self {
        case .online: "Online"
        case .offline: "Offline"
        case .requiresConnection: "Connection Required"
        }
    }

    fileprivate init(pathStatus: NWPath.Status) {
        switch pathStatus {
        case .satisfied:
            self = .online
        case .unsatisfied:
            self = .offline
        case .requiresConnection:
            self = .requiresConnection
        @unknown default:
            self = .offline
        }
    }
}

@MainActor
public final class ReachabilityMonitor: ObservableObject {
    @Published public private(set) var status: ReachabilityStatus

    private let monitor: NWPathMonitor
    private let queue = DispatchQueue(label: "com.matthewyjiang.tsk.reachability")
    private var started = false

    public init(initialStatus: ReachabilityStatus = .online, monitor: NWPathMonitor = NWPathMonitor()) {
        self.status = initialStatus
        self.monitor = monitor
    }

    public func start() {
        guard !started else { return }
        started = true
        monitor.pathUpdateHandler = { [weak self] path in
            let status = ReachabilityStatus(pathStatus: path.status)
            Task { @MainActor in
                self?.status = status
            }
        }
        monitor.start(queue: queue)
    }

    public func stop() {
        guard started else { return }
        monitor.cancel()
        started = false
    }

    deinit {
        monitor.cancel()
    }
}
