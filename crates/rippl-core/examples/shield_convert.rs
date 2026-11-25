//! Converts ABP filter lists to WebKit JSON, for inspecting Shield's output:
//!   cargo run --release -p rippl-core --example shield_convert -- easylist.txt > out.json
use std::time::Instant;

fn main() {
    let texts: Vec<String> = std::env::args().skip(1).map(|p| std::fs::read_to_string(p).expect("read list")).collect();
    let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
    let start = Instant::now();
    let (json, stats) = rippl_core::shield::convert(&refs, &[]);
    eprintln!("{stats:?} rules={} json={} KB in {:?}", stats.rules(), json.len() / 1024, start.elapsed());
    print!("{json}");
}
