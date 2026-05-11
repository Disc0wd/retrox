// ============================================================
// RetroX CLI (GN-Z11)
// Entry point. Parses and renders .rnmdl files.
// Rust 1.95.0 | Edition 2021 | FROZEN
//
// Usage:
//   retrox render <file.rnmdl>
//   retrox validate <file.rnmdl>
//   retrox check <file.rnmdl>
//   retrox help
// ============================================================

mod lexer;
mod ast;
mod parser;
mod validator;
mod renderer;
mod gui;
mod image;
mod font;
mod platform;

use std::env;
use std::fs;
use std::process;

use lexer::Lexer;
use parser::Parser;
use validator::Validator;
use renderer::Renderer;

const VERSION:  &str = "0.0.0 (GN-Z11)";
const RESET:    &str = "\x1b[0m";
const BOLD:     &str = "\x1b[1m";
const RED:      &str = "\x1b[31m";
const GREEN:    &str = "\x1b[32m";
const YELLOW:   &str = "\x1b[33m";
const CYAN:     &str = "\x1b[36m";
const DARK_GRAY:&str = "\x1b[90m";

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        process::exit(1);
    }

    match args[1].as_str() {
        "render" | "r" => {
            if args.len() < 3 {
                eprintln!("{}{}Error:{} 'render' requires a file path.", BOLD, RED, RESET);
                eprintln!("  Usage: retrox render <file.rnmdl>");
                process::exit(1);
            }
            cmd_render(&args[2], true, false);
        }

        "validate" | "v" => {
            if args.len() < 3 {
                eprintln!("{}{}Error:{} 'validate' requires a file path.", BOLD, RED, RESET);
                eprintln!("  Usage: retrox validate <file.rnmdl>");
                process::exit(1);
            }
            cmd_validate(&args[2]);
        }

        "check" | "c" => {
            if args.len() < 3 {
                eprintln!("{}{}Error:{} 'check' requires a file path.", BOLD, RED, RESET);
                eprintln!("  Usage: retrox check <file.rnmdl>");
                process::exit(1);
            }
            // Render without meta info (clean output)
            cmd_render(&args[2], false, true);
        }

        "help" | "--help" | "-h" => {
            print_usage();
        }

        "version" | "--version" | "-V" => {
            println!("{}{}RetroX RNMDL Parser & Renderer{}", BOLD, CYAN, RESET);
            println!("  Version:  {}", VERSION);
            println!("  Language: Rust 1.95.0");
            println!("  Edition:  2021");
            println!("  Spec:     RNMDL GN-Z11 (v0.0.0)");
        }
        
        "gui" | "g" => {
            if args.len() < 3 {
            eprintln!("{}{}Error:{} 'gui' requires a file path.", BOLD, RED, RESET);
            eprintln!("  Usage: retrox gui <file.rnmdl>");
            process::exit(1);
            }
            cmd_gui(&args[2]);
        }

        unknown => {
            eprintln!("{}{}Error:{} Unknown command '{}'.", BOLD, RED, RESET, unknown);
            print_usage();
            process::exit(1);
        }
    }
}

// ─── Commands ─────────────────────────────────────────────

fn cmd_render(path: &str, show_meta: bool, show_comments: bool) {
    let source = read_file(path);

    // Lex
    let mut lexer = Lexer::new(&source);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(e) => {
            print_lex_error(&e, path, &source);
            process::exit(1);
        }
    };

    // Parse
    let mut parser = Parser::new(tokens);
    let ast = match parser.parse() {
        Ok(a) => a,
        Err(e) => {
            print_parse_error(&e, path, &source);
            process::exit(1);
        }
    };

    // Validate
    let validator = Validator::new();
    let errors = validator.validate(&ast);
    if !errors.is_empty() {
        print_validation_errors(&errors, path, &source);
        process::exit(1);
    }

    // Render
    let renderer = Renderer::new()
        .with_meta(show_meta)
        .with_comments(show_comments);

    let output = renderer.render(&ast);
    print!("{}", output);
}

fn cmd_validate(path: &str) {
    let source = read_file(path);

    // Lex
    let mut lexer = Lexer::new(&source);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(e) => {
            print_lex_error(&e, path, &source);
            process::exit(1);
        }
    };

    // Parse
    let mut parser = Parser::new(tokens);
    let ast = match parser.parse() {
        Ok(a) => a,
        Err(e) => {
            print_parse_error(&e, path, &source);
            process::exit(1);
        }
    };

    // Validate
    let validator = Validator::new();
    let errors = validator.validate(&ast);

    if errors.is_empty() {
        println!("{}{}✓ Valid{} — {}", BOLD, GREEN, RESET, path);
        println!("  {}Document passed all GN-Z11 validation rules.{}", DARK_GRAY, RESET);
    } else {
        print_validation_errors(&errors, path, &source);
        process::exit(1);
    }
}

// ─── File Reading ──────────────────────────────────────────

fn read_file(path: &str) -> String {
    if !path.ends_with(".rnmdl") {
        eprintln!(
            "{}{}Warning:{} File '{}' does not have .rnmdl extension.",
            BOLD, YELLOW, RESET, path
        );
    }

    match fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("{}{}Error:{} Cannot read file '{}': {}", BOLD, RED, RESET, path, e);
            process::exit(1);
        }
    }
}

// ─── Error Display ─────────────────────────────────────────

fn print_lex_error(e: &lexer::LexError, path: &str, source: &str) {
    eprintln!("\n{}{}✗ Lex Error{} in {}\n", BOLD, RED, RESET, path);
    eprintln!("  {}", e);
    print_source_context(source, e.line, e.col);
}

fn print_parse_error(e: &parser::ParseError, path: &str, source: &str) {
    eprintln!("\n{}{}✗ Parse Error{} in {}\n", BOLD, RED, RESET, path);
    eprintln!("  {}", e);
    print_source_context(source, e.line, e.col);
}

fn print_validation_errors(
    errors: &[validator::ValidationError],
    path: &str,
    source: &str,
) {
    eprintln!(
        "\n{}{}✗ Validation Failed{} — {} — {} error{}\n",
        BOLD, RED, RESET, path,
        errors.len(),
        if errors.len() == 1 { "" } else { "s" }
    );

    for (i, err) in errors.iter().enumerate() {
        eprintln!("  {}{}[{}]{} {}", BOLD, RED, i + 1, RESET, err.message);
        if err.line > 0 {
            print_source_context(source, err.line, 1);
        }
    }

    eprintln!(
        "\n  {}Document rejected. Fix all errors and try again.{}",
        YELLOW, RESET
    );
}

fn print_source_context(source: &str, line: usize, col: usize) {
    let lines: Vec<&str> = source.lines().collect();

    // Print line before (context)
    if line >= 2 {
        if let Some(prev) = lines.get(line - 2) {
            eprintln!("  {}  {:>4} │ {}{}", DARK_GRAY, line - 1, prev, RESET);
        }
    }

    // Print error line
    if let Some(err_line) = lines.get(line - 1) {
        eprintln!("  {}{}{:>4} │ {}{}", BOLD, RED, line, err_line, RESET);

        // Print column pointer
        if col > 0 {
            let pointer = " ".repeat(col + 7);
            eprintln!("  {}{}{}^{}", BOLD, RED, pointer, RESET);
        }
    }

    // Print line after (context)
    if let Some(next) = lines.get(line) {
        eprintln!("  {}  {:>4} │ {}{}", DARK_GRAY, line + 1, next, RESET);
    }

    eprintln!();
}

fn cmd_gui(path: &str) {
    let mut browser = gui::browser::Browser::new("RetroX", 900, 650);
    if let Err(e) = browser.load(path) {
        eprintln!("{}{}Error:{} {}", BOLD, RED, RESET, e);
        process::exit(1);
    }
    browser.run();
}
// ─── Help ──────────────────────────────────────────────────

fn print_usage() {
    println!("{}{}RetroX RNMDL Parser & Renderer{}", BOLD, CYAN, RESET);
    println!("{}Version:{} {}", DARK_GRAY, RESET, VERSION);
    println!();
    println!("{}USAGE:{}", BOLD, RESET);
    println!("  retrox <command> [file]");
    println!();
    println!("{}COMMANDS:{}", BOLD, RESET);
    println!("  {}render{}   <file.rnmdl>   Parse, validate and render a document", GREEN, RESET);
    println!("  {}validate{} <file.rnmdl>   Validate without rendering", GREEN, RESET);
    println!("  {}gui{}      <file.rnmdl>   Open graphical browser", GREEN, RESET);
    println!("  {}check{}    <file.rnmdl>   Render without meta information", GREEN, RESET);
    println!("  {}version{}                 Show version information", GREEN, RESET);
    println!("  {}help{}                    Show this help message", GREEN, RESET);
    println!();
    println!("{}EXAMPLES:{}", BOLD, RESET);
    println!("  retrox render site.rnmdl");
    println!("  retrox validate site.rnmdl");
    println!("  retrox check site.rnmdl");
    println!();
    println!("{}SPEC:{}", BOLD, RESET);
    println!("  Language:  RNMDL GN-Z11 (v0.0.0)");
    println!("  Modules:   text, images");
    println!("  Tags:      container, section, h1, h2, h3, heading, paragraph, p, image");
}
