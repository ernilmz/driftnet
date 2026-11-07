import Foundation
import CFNetwork
final class ResultBox { var proxies: [[String:Any]]?; var failed = false }
let targets = ["https://discord.com/api/v10/gateway", "https://example.org/"]
guard let settings = CFNetworkCopySystemProxySettings()?.takeRetainedValue() else {fatalError("No system proxy settings")}
for target in targets {
    let targetURL = URL(string: target)!
    let initial = CFNetworkCopyProxiesForURL(targetURL as CFURL, settings).takeRetainedValue() as NSArray
    guard let first = initial.firstObject as? [String:Any] else {fatalError("No proxy result")}
    let box = ResultBox()
    if first[kCFProxyTypeKey as String] as? String == kCFProxyTypeAutoConfigurationURL as String,
       let pacURL = first[kCFProxyAutoConfigurationURLKey as String] as? URL {
        var context = CFStreamClientContext(version:0, info:Unmanaged.passUnretained(box).toOpaque(), retain:nil, release:nil, copyDescription:nil)
        let source = CFNetworkExecuteProxyAutoConfigurationURL(pacURL as CFURL, targetURL as CFURL, {info,proxies,error in
            let box = Unmanaged<ResultBox>.fromOpaque(info).takeUnretainedValue()
            box.proxies = proxies as? [[String:Any]]; box.failed = error != nil
            CFRunLoopStop(CFRunLoopGetCurrent())
        }, &context)
        CFRunLoopAddSource(CFRunLoopGetCurrent(),source,.defaultMode)
        CFRunLoopRunInMode(.defaultMode,10,false)
        CFRunLoopSourceInvalidate(source)
    } else {box.proxies = initial as? [[String:Any]]}
    guard !box.failed, let proxy = box.proxies?.first else {fatalError("PAC evaluation failed")}
    let kind = proxy[kCFProxyTypeKey as String] as? String ?? "unknown"
    print("\(targetURL.host!): \(kind)")
    if targetURL.host == "discord.com" {assert(kind == kCFProxyTypeHTTP as String || kind == kCFProxyTypeHTTPS as String)}
    else {assert(kind == kCFProxyTypeNone as String)}
}
