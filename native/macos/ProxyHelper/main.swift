import Foundation
import SystemConfiguration
import Darwin

// Privileged preferences editor. No sockets, traffic parsing, shell or arbitrary paths.
let directory = "/Library/Application Support/Drift"
let journalURL = URL(fileURLWithPath: directory + "/proxy-journal.json")
struct Previous: Codable { let service: String; let enabled: Data?; let url: Data? }
struct Backup: Codable { let url: String; let previous: [Previous] }
func error(_ text: String) -> NSError { NSError(domain: "app.drift.network", code: 1, userInfo: [NSLocalizedDescriptionKey: text]) }
func encode(_ value: Any?) throws -> Data? {
    guard let value = value else { return nil }
    return try PropertyListSerialization.data(fromPropertyList: ["value":value], format: .binary, options: 0)
}
func decode(_ value: Data?) throws -> Any? {
    guard let value = value else { return nil }
    return (try PropertyListSerialization.propertyList(from: value, options: [], format: nil) as? [String:Any])?["value"]
}
func perform(_ action: String, _ expectedURL: String?) throws {
    guard geteuid() == 0 else { throw error("Install the Drift networking helper first.") }
    guard let prefs = SCPreferencesCreate(nil, "Drift" as CFString, nil), SCPreferencesLock(prefs, true) else {throw error("Could not lock network preferences.")}
    defer { SCPreferencesUnlock(prefs) }
    guard let services = SCNetworkServiceCopyAll(prefs) as? [SCNetworkService] else {throw error("No network services are available.")}
    let enabledKey = kSCPropNetProxiesProxyAutoConfigEnable as String
    let urlKey = kSCPropNetProxiesProxyAutoConfigURLString as String
    if action == "apply" {
        guard let endpoint = expectedURL, let url = URLComponents(string: endpoint), url.scheme == "http", url.host == "127.0.0.1", (url.port ?? 0) >= 1024, url.path == "/proxy.pac", url.user == nil, url.password == nil else {throw error("Invalid local PAC endpoint.")}
        guard !FileManager.default.fileExists(atPath: journalURL.path) else {throw error("Previous website routing must be recovered first.")}
        var previous = [Previous]()
        var changes = [(SCNetworkProtocol,[String:Any])]()
        for service in services {
            guard SCNetworkServiceGetEnabled(service), let proto = SCNetworkServiceCopyProtocol(service, kSCNetworkProtocolTypeProxies), let id = SCNetworkServiceGetServiceID(service) else {continue}
            var config = SCNetworkProtocolGetConfiguration(proto) as? [String:Any] ?? [:]
            for key in [enabledKey,"HTTPEnable","HTTPSEnable","SOCKSEnable","ProxyAutoDiscoveryEnable"] {
                if (config[key] as? NSNumber)?.boolValue == true {throw error("Another proxy or automatic proxy discovery is active. Drift kept it unchanged.")}
            }
            previous.append(Previous(service: id as String, enabled: try encode(config[enabledKey]), url: try encode(config[urlKey])))
            config[enabledKey] = 1; config[urlKey] = endpoint; changes.append((proto,config))
        }
        guard !changes.isEmpty else {throw error("No enabled network service is available.")}
        let data = try JSONEncoder().encode(Backup(url: endpoint, previous: previous))
        try data.write(to: journalURL, options: [.atomic])
        try FileManager.default.setAttributes([.posixPermissions:0o600], ofItemAtPath:journalURL.path)
        let descriptor = Darwin.open(journalURL.path, O_RDONLY)
        if descriptor >= 0 { _ = fsync(descriptor); Darwin.close(descriptor) }
        for (proto,config) in changes {guard SCNetworkProtocolSetConfiguration(proto, config as CFDictionary) else {throw error("Could not configure selected websites.")}}
    } else {
        guard FileManager.default.fileExists(atPath: journalURL.path) else {return}
        let backup = try JSONDecoder().decode(Backup.self, from: Data(contentsOf: journalURL))
        if let expectedURL = expectedURL, expectedURL != backup.url {throw error("Website routing belongs to a different session.")}
        for previous in backup.previous {
            guard let service = SCNetworkServiceCopy(prefs, previous.service as CFString), let proto = SCNetworkServiceCopyProtocol(service, kSCNetworkProtocolTypeProxies) else {continue}
            var config = SCNetworkProtocolGetConfiguration(proto) as? [String:Any] ?? [:]
            // Preserve proxy settings changed by the user/another app during the lease.
            guard config[urlKey] as? String == backup.url else {continue}
            config[enabledKey] = try decode(previous.enabled)
            config[urlKey] = try decode(previous.url)
            guard SCNetworkProtocolSetConfiguration(proto, config as CFDictionary) else {throw error("Could not restore website routing.")}
        }
    }
    guard SCPreferencesCommitChanges(prefs), SCPreferencesApplyChanges(prefs) else {throw error("Could not apply network preferences.")}
    if action != "apply" {try FileManager.default.removeItem(at: journalURL)}
}
do {
    guard CommandLine.arguments.count >= 2 else {throw error("Missing helper action.")}
    let action = CommandLine.arguments[1]
    guard ["apply","restore","recover"].contains(action) else {throw error("Invalid helper action.")}
    try perform(action, CommandLine.arguments.count > 2 ? CommandLine.arguments[2] : nil)
} catch {
    FileHandle.standardError.write(Data((error.localizedDescription + "\n").utf8)); exit(1)
}
