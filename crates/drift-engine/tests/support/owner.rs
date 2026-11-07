use std::io::Write;
fn main() {
    let binary = std::env::var_os("DRIFT_ENGINE_TEST_BINARY").unwrap();
    let engine = drift_engine::Engine::start(std::path::Path::new(&binary)).unwrap();
    println!("{}", engine.endpoint());
    std::io::stdout().flush().unwrap();
    loop {
        std::thread::park();
    }
}
