//! Build step 1: send one frame to Jev and print its answer.
//! Usage: cargo run --example jev_probe -- <image.jpg> "<phrase>" ["<phrase>" ...]

use splitseek_lib::jev::Jev;
use std::time::Instant;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: jev_probe <image.jpg> \"<phrase>\" [\"<phrase>\" ...]");
        std::process::exit(2);
    }
    let jpeg = std::fs::read(&args[0]).unwrap_or_else(|e| {
        eprintln!("can't read {}: {e}", args[0]);
        std::process::exit(1);
    });
    let jev = Jev::from_env().unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1);
    });

    println!("image: {} ({} KB), requested model: {}", args[0], jpeg.len() / 1024, jev.model());
    for phrase in &args[1..] {
        let start = Instant::now();
        match jev.score_jpeg(&jpeg, phrase).await {
            Ok(s) => println!(
                "{:>5.1}%  \"{phrase}\"  ({} ms, {} input tokens, {})",
                s.noul * 100.0,
                start.elapsed().as_millis(),
                s.input_tokens,
                s.model
            ),
            Err(e) => println!("ERROR  \"{phrase}\"  {e}"),
        }
    }
}
