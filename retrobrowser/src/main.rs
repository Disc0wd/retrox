// ============================================================
// RetroBrowser
// Graphical RNMDL browser for the RetroX ecosystem.
// Rust 1.95.0 | Edition 2021 | FROZEN at GN-Z11
// ============================================================

mod gui;
mod image;
mod font;
mod platform;

use std::env;
use std::process;

const BOLD:  &str = "\x1b[1m";
const RED:   &str = "\x1b[31m";
const CYAN:  &str = "\x1b[36m";
const RESET: &str = "\x1b[0m";

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        process::exit(1);
    }

    match args[1].as_str() {
        "gui" | "g" => {
            if args.len() < 3 {
                eprintln!("{}{}Error:{} 'gui' requires a file path.", BOLD, RED, RESET);
                eprintln!("  Usage: retrobrowser gui <file.rnmdl>");
                process::exit(1);
            }
            cmd_gui(&args[2]);
        }
        "help" | "--help" | "-h" => print_usage(),
        "version" | "--version" | "-V" => {
            println!("{}{}RetroBrowser{} v0.0.0 (GN-Z11)", BOLD, CYAN, RESET);
        }
        unknown => {
            eprintln!("{}{}Error:{} Unknown command '{}'.", BOLD, RED, RESET, unknown);
            print_usage();
            process::exit(1);
        }
    }
}

fn cmd_gui(path: &str) {
    let mut browser = gui::browser::Browser::new("RetroBrowser", 900, 650);
    if let Err(e) = browser.load(path) {
        eprintln!("{}{}Error:{} {}", BOLD, RED, RESET, e);
        process::exit(1);
    }
    browser.run();
}

fn print_usage() {
    println!("{}{}RetroBrowser{} — RNMDL GN-Z11 graphical browser", BOLD, CYAN, RESET);
    println!();
    println!("{}USAGE:{}", BOLD, RESET);
    println!("  retrobrowser gui <file.rnmdl>");
    println!();
    println!("{}COMMANDS:{}", BOLD, RESET);
    println!("  gui <file.rnmdl>   Open document in graphical browser");
    println!("  version            Show version");
    println!("  help               Show this message");
}