import Foundation
import Network
import NetworkExtension

final class TCPRelay {
    private let flow: NEAppProxyTCPFlow
    private let channel: SocksChannel
    private let host: String
    private let port: UInt16
    private var finished = false
    private var upstreamEnded = false
    private var downstreamEnded = false
    private let onClose: () -> Void
    init(flow: NEAppProxyTCPFlow, enginePort: UInt16, host: String, port: UInt16, queue: DispatchQueue, onClose: @escaping () -> Void) {
        self.flow = flow; self.host = host; self.port = port; self.onClose = onClose
        channel = SocksChannel(port: enginePort, queue: queue)
    }
    func start() {
        channel.queue.async {
            guard !self.finished else { return }
            self.channel.open(host: self.host, port: self.port) { error in
                guard error == nil else { self.close(error); return }
                self.flow.open(withLocalEndpoint: nil) { error in
                    self.channel.queue.async {
                        guard !self.finished else { return }
                        guard error == nil else { self.close(error); return }
                        self.readFromApp(); self.readFromNetwork()
                    }
                }
            }
        }
    }
    func stop() { channel.queue.async { self.close(SocksChannel.error("Drift access was stopped.")) } }
    private func close(_ error: Error?) {
        guard !finished else { return }
        finished = true
        channel.cancel()
        flow.closeReadWithError(error); flow.closeWriteWithError(error)
        onClose()
    }
    private func finishIfBothEnded() {
        if upstreamEnded && downstreamEnded { close(nil) }
    }
    private func readFromApp() {
        guard !finished, !upstreamEnded else { return }
        flow.readData { data, error in
            self.channel.queue.async {
                guard !self.finished else { return }
                guard error == nil else { self.close(error); return }
                guard let data = data, !data.isEmpty else {
                    self.upstreamEnded = true
                    self.channel.connection.send(content: nil, contentContext: .finalMessage, isComplete: true, completion: .contentProcessed { error in
                        if let error = error { self.close(error) } else { self.finishIfBothEnded() }
                    }); return
                }
                self.channel.send(data) { error in
                    guard error == nil else { self.close(error); return }
                    self.readFromApp()
                }
            }
        }
    }
    private func readFromNetwork() {
        guard !finished, !downstreamEnded else { return }
        channel.connection.receive(minimumIncompleteLength: 1, maximumLength: 64 * 1024) { data, _, ended, error in
            guard !self.finished else { return }
            guard error == nil else { self.close(error); return }
            let next = {
                if ended {
                    self.downstreamEnded = true
                    self.flow.closeWriteWithError(nil)
                    self.finishIfBothEnded()
                } else { self.readFromNetwork() }
            }
            if let data = data, !data.isEmpty {
                self.flow.write(data) { error in
                    self.channel.queue.async {
                        guard !self.finished else { return }
                        if let error = error { self.close(error) } else { next() }
                    }
                }
            } else { next() }
        }
    }
}
