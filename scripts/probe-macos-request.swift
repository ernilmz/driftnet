import Foundation
let configuration = URLSessionConfiguration.ephemeral
configuration.timeoutIntervalForRequest = 20
configuration.timeoutIntervalForResource = 25
let session = URLSession(configuration: configuration)
let semaphore = DispatchSemaphore(value: 0)
var passed = false
session.dataTask(with: URL(string: "https://discord.com/api/v10/gateway")!) { data,response,error in
    let code = (response as? HTTPURLResponse)?.statusCode ?? 0
    passed = code == 200 && error == nil
    print("macOS URLSession using system proxy: HTTP \(code)")
    if let error = error {print(error.localizedDescription)}
    semaphore.signal()
}.resume()
if semaphore.wait(timeout: .now() + 30) == .timedOut { print("Request timed out"); exit(1) }
exit(passed ? 0 : 1)
