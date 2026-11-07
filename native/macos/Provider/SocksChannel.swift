import Foundation
import Network

// One bounded, backpressured connection. All callbacks run on the supplied queue.
final class SocksChannel {
    let connection: NWConnection
    let queue: DispatchQueue
    private var completion: ((Error?) -> Void)?
    private var deadline: DispatchWorkItem?
    private var closed = false
    init(port: UInt16, queue: DispatchQueue) {
        self.queue = queue
        connection = NWConnection(host: "127.0.0.1", port: NWEndpoint.Port(rawValue: port)!, using: .tcp)
    }
    static func error(_ message: String) -> NSError {
        NSError(domain: "app.drift.desktop.network", code: 1, userInfo: [NSLocalizedDescriptionKey: message])
    }
    func open(host: String?, port: UInt16 = 0, completion: @escaping (Error?) -> Void) {
        self.completion = completion
        let timeout = DispatchWorkItem { [weak self] in self?.finish(Self.error("The engine handshake timed out.")) }
        deadline = timeout
        queue.asyncAfter(deadline: .now() + 10, execute: timeout)
        connection.stateUpdateHandler = { [weak self] state in
            guard let self = self, !self.closed else { return }
            switch state {
            case .ready:
                self.send(Data([5, 1, 0])) { error in
                    guard error == nil else { self.finish(error); return }
                    self.readExactly(2) { reply, error in
                        guard error == nil, reply == Data([5, 0]) else {
                            self.finish(error ?? Self.error("The engine refused SOCKS5.")); return
                        }
                        guard let host = host else { self.finish(nil); return }
                        guard let request = Self.request(host: host, port: port) else {
                            self.finish(Self.error("Invalid flow destination.")); return
                        }
                        self.send(request) { error in
                            guard error == nil else { self.finish(error); return }
                            self.readExactly(4) { header, error in
                                guard error == nil, let header = header, header[0] == 5, header[1] == 0 else {
                                    self.finish(error ?? Self.error("The engine could not connect.")); return
                                }
                                switch header[3] {
                                case 1: self.consumeAddress(6)
                                case 4: self.consumeAddress(18)
                                case 3:
                                    self.readExactly(1) { count, error in
                                        guard error == nil, let count = count else { self.finish(error); return }
                                        self.consumeAddress(Int(count[0]) + 2)
                                    }
                                default: self.finish(Self.error("Invalid SOCKS5 reply."))
                                }
                            }
                        }
                    }
                }
            case .failed(let error): self.finish(error)
            case .cancelled:
                if self.completion != nil { self.finish(Self.error("The engine connection closed.")) }
            default: break
            }
        }
        connection.start(queue: queue)
    }
    private func consumeAddress(_ count: Int) {
        readExactly(count) { _, error in self.finish(error) }
    }
    private func finish(_ error: Error?) {
        guard let completion = completion else { return }
        self.completion = nil
        deadline?.cancel(); deadline = nil
        if error != nil { cancel() }
        completion(error)
    }
    func cancel() {
        closed = true
        completion = nil
        connection.stateUpdateHandler = nil
        deadline?.cancel(); deadline = nil
        connection.cancel()
    }
    func send(_ bytes: Data, completion: @escaping (Error?) -> Void) {
        connection.send(content: bytes, completion: .contentProcessed { completion($0) })
    }
    private func readExactly(_ count: Int, bytes: Data = Data(), completion: @escaping (Data?, Error?) -> Void) {
        guard count > bytes.count else { completion(bytes, nil); return }
        connection.receive(minimumIncompleteLength: 1, maximumLength: count - bytes.count) { data, _, ended, error in
            var result = bytes
            if let data = data { result.append(data) }
            if result.count == count { completion(result, nil) }
            else if error != nil || ended { completion(nil, error ?? Self.error("Incomplete SOCKS5 response.")) }
            else { self.readExactly(count, bytes: result, completion: completion) }
        }
    }
    static func request(host: String, port: UInt16) -> Data? {
        guard port != 0 else { return nil }
        var result = Data([5, 1, 0])
        if let address = IPv4Address(host) { result.append(1); result.append(address.rawValue) }
        else if let address = IPv6Address(host) { result.append(4); result.append(address.rawValue) }
        else {
            guard !host.contains(where: { $0.isWhitespace || "/@?#\\[]:".contains($0) }) else { return nil }
            guard let ascii = URL(string: "https://\(host)")?.host,
                  !ascii.isEmpty, ascii.utf8.count <= 255,
                  !ascii.contains("/"), !ascii.contains("@") else { return nil }
            let name = Data(ascii.utf8)
            result.append(3); result.append(UInt8(name.count)); result.append(name)
        }
        result.append(UInt8(port >> 8)); result.append(UInt8(port & 255))
        return result
    }
}
