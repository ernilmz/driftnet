import Foundation
import NetworkExtension
import Network

@objc(DriftTransparentProxy)
final class DriftTransparentProxy: NETransparentProxyProvider {
    private let lock = NSLock()
    private let queue = DispatchQueue(label: "app.drift.desktop.network.relay")
    private var policy: UnsafeMutableRawPointer?
    private var ownerUID: UInt32 = UInt32.max
    private var enginePort: UInt16 = 0
    private var enginePID: Int32 = 0
    private var running = false
    private var sessions: [UUID: TCPRelay] = [:]
    private var health: DispatchSourceTimer?
    private var probe: SocksChannel?
    override func startProxy(options: [String: Any]? = nil, completionHandler: @escaping (Error?) -> Void) {
        guard let config = (protocolConfiguration as? NETunnelProviderProtocol)?.providerConfiguration,
              let rules = config["rules"] as? Data,
              let port = config["enginePort"] as? NSNumber, (1024...65535).contains(port.intValue),
              let uid = config["ownerUID"] as? NSNumber, uid.uint32Value != UInt32.max,
              let pid = config["enginePID"] as? NSNumber, pid.int32Value > 0 else {
            completionHandler(SocksChannel.error("Invalid Drift network configuration.")); return
        }
        guard let newPolicy = rules.withUnsafeBytes({ bytes in drift_policy_create(bytes.bindMemory(to: UInt8.self).baseAddress, rules.count) }) else {
            completionHandler(SocksChannel.error("Invalid Drift routing policy.")); return
        }
        lock.lock()
        if policy != nil { lock.unlock(); drift_policy_destroy(newPolicy); completionHandler(SocksChannel.error("Drift is already starting.")); return }
        policy = newPolicy; ownerUID = uid.uint32Value; enginePort = port.uint16Value; enginePID = pid.int32Value
        lock.unlock()
        let settings = NETransparentProxyNetworkSettings(tunnelRemoteAddress: "127.0.0.1")
        // Unknown flows return false below and keep their original OS route.
        // UDP is deliberately excluded until a verified UDP adapter exists.
        settings.includedNetworkRules = [NENetworkRule(remoteNetwork: nil, remotePrefix: 0, localNetwork: nil, localPrefix: 0, protocol: .TCP, direction: .outbound)]
        settings.excludedNetworkRules = []
        queue.async {
            let check = SocksChannel(port: port.uint16Value, queue: self.queue)
            self.probe = check
            check.open(host: nil) { error in
                check.cancel(); self.probe = nil
                guard error == nil else { self.releasePolicy(); completionHandler(error); return }
                self.setTunnelNetworkSettings(settings) { error in
                    if error == nil {
                        self.lock.lock(); self.running = true; self.lock.unlock()
                        self.startHealthChecks(port: port.uint16Value)
                    } else { self.releasePolicy() }
                    completionHandler(error)
                }
            }
        }
    }
    override func handleNewFlow(_ flow: NEAppProxyFlow) -> Bool {
        guard let tcp = flow as? NEAppProxyTCPFlow,
              let endpoint = tcp.remoteEndpoint as? NWHostEndpoint,
              let destinationPort = UInt16(endpoint.port), destinationPort > 0,
              let audit = flow.metaData.sourceAppAuditToken else { return false }
        let uid = audit.withUnsafeBytes { drift_audit_euid($0.baseAddress, audit.count) }
        let pid = audit.withUnsafeBytes { drift_audit_pid($0.baseAddress, audit.count) }
        let signingID = flow.metaData.sourceAppSigningIdentifier
        lock.lock(); defer { lock.unlock() }
        guard running, uid == ownerUID, pid != enginePID,
              !signingID.hasPrefix("app.drift.desktop"), let policy = policy,
              sessions.count < 256 else { return false }
        let app = Data("macos:\(signingID)".utf8)
        let hostname = flow.remoteHostname.map { Data($0.utf8) } ?? Data()
        let selected = app.withUnsafeBytes { appBytes in
            hostname.withUnsafeBytes { hostBytes in
                drift_policy_selected(policy, appBytes.bindMemory(to: UInt8.self).baseAddress, app.count,
                    hostBytes.bindMemory(to: UInt8.self).baseAddress, hostname.count) == 1
            }
        }
        guard selected else { return false }
        // A hostname supplied by the OS is trusted for policy; an IP alone is
        // never promoted into a domain rule. No global shared-CDN IP rules.
        let destination = flow.remoteHostname ?? endpoint.hostname
        let id = UUID()
        let relay = TCPRelay(flow: tcp, enginePort: enginePort, host: destination, port: destinationPort, queue: queue) { [weak self] in
            guard let self = self else { return }
            self.lock.lock(); self.sessions.removeValue(forKey: id); self.lock.unlock()
        }
        sessions[id] = relay; relay.start(); return true
    }
    override func stopProxy(with reason: NEProviderStopReason, completionHandler: @escaping () -> Void) {
        lock.lock(); running = false; let active = Array(sessions.values); sessions.removeAll(); lock.unlock()
        queue.async {
            self.health?.cancel(); self.health = nil; self.probe?.cancel(); self.probe = nil
            for relay in active { relay.stop() }
            self.releasePolicy(); completionHandler()
        }
    }
    private func releasePolicy() {
        lock.lock(); let old = policy; policy = nil; running = false; lock.unlock()
        if let old = old { drift_policy_destroy(old) }
    }
    private func startHealthChecks(port: UInt16) {
        queue.async { [self] in
            let timer = DispatchSource.makeTimerSource(queue: self.queue)
            timer.schedule(deadline: .now() + 2, repeating: 2)
            timer.setEventHandler { [weak self] in
                guard let self = self, self.probe == nil else { return }
                let check = SocksChannel(port: port, queue: self.queue); self.probe = check
                check.open(host: nil) { error in
                    check.cancel(); self.probe = nil
                    if let error = error {
                        self.lock.lock(); self.running = false; self.lock.unlock()
                        self.cancelProxyWithError(error)
                    }
                }
            }
            self.health = timer; timer.resume()
        }
    }
}
