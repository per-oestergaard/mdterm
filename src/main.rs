mod config;
mod diagram;
mod export;
mod image;
mod json;
mod markdown;
mod style;
mod theme;
mod viewer;

use std::io::{self, IsTerminal, Read};
use std::{fs, process};

use clap::Parser;

#[derive(Parser)]
#[command(
    name = "mdterm",
    version,
    about = "Terminal Markdown viewer with style"
)]
struct Cli {
    /// Markdown file(s) to view
    files: Vec<String>,

    /// Theme: dark or light
    #[arg(long, short = 'T')]
    theme: Option<String>,

    /// Display width override (0 = auto)
    #[arg(long, short = 'w', default_value = "0")]
    width: usize,

    /// Slide mode (horizontal rules become slide separators)
    #[arg(long, short = 's')]
    slides: bool,

    /// Deprecated: file watching is now always active
    #[arg(long, short = 'f', hide = true)]
    follow: bool,

    /// Show line numbers in code blocks
    #[arg(long, short = 'l')]
    line_numbers: bool,

    /// Export format instead of interactive view (html, svg, png, odp, odp+svg, odp+png)
    #[arg(long)]
    export: Option<String>,

    /// Output path prefix for image exports (e.g. ./out/slide_)
    /// Required with --export svg or --export png.
    /// Files are written as <prefix>0001.svg / <prefix>0001.png etc.
    #[arg(long)]
    export_prefix: Option<String>,

    /// Output file path for single-file exports (e.g. ./out/presentation.odp)
    /// Required with --export odp / odp+svg / odp+png.
    #[arg(long)]
    export_file: Option<String>,

    /// Disable colors
    #[arg(long)]
    no_color: bool,
}

fn main() {
    let cli = Cli::parse();
    let config = config::Config::load();

    // Determine theme
    let theme_name = cli.theme.as_deref().unwrap_or(&config.theme);
    let initial_theme = match theme_name {
        "light" => theme::Theme::light(),
        _ => theme::Theme::dark(),
    };

    let line_numbers = cli.line_numbers || config.line_numbers;
    let width = if cli.width > 0 {
        cli.width
    } else if config.width > 0 {
        config.width
    } else {
        0
    };

    // Read content: stdin or file(s)
    let (content, filename) = if cli.files.is_empty() {
        if io::stdin().is_terminal() {
            eprintln!("Usage: mdterm [OPTIONS] <FILE>...");
            eprintln!("       command | mdterm");
            eprintln!();
            eprintln!("Try 'mdterm --help' for more information.");
            process::exit(1);
        }
        const MAX_STDIN_BYTES: u64 = 100 * 1024 * 1024; // 100 MB
        let mut buf = String::new();
        let n = io::stdin()
            .take(MAX_STDIN_BYTES + 1)
            .read_to_string(&mut buf)
            .unwrap_or_else(|e| {
                eprintln!("Error reading stdin: {}", e);
                process::exit(1);
            });
        if n as u64 > MAX_STDIN_BYTES {
            eprintln!("Error: stdin input exceeds 100 MB limit");
            process::exit(1);
        }
        (buf, "<stdin>".to_string())
    } else {
        let path = &cli.files[0];
        let c = fs::read_to_string(path).unwrap_or_else(|e| {
            eprintln!("Error reading '{}': {}", path, e);
            process::exit(1);
        });
        (c, path.clone())
    };

    let is_json = filename.ends_with(".json");

    // Export mode
    if let Some(ref fmt) = cli.export {
        match fmt.as_str() {
            "html" => {
                let w = if width > 0 { width } else { 80 };
                export::to_html(&content, w, &initial_theme, &filename);
            }
            "svg" | "png" => {
                let prefix = cli.export_prefix.as_deref().unwrap_or_else(|| {
                    eprintln!("Error: --export-prefix is required with --export {}", fmt);
                    process::exit(1);
                });
                let w = if width > 0 { width } else { 120 };
                if fmt == "svg" {
                    export::export_slides_svg(&content, w, &initial_theme, prefix, cli.slides);
                } else {
                    export::export_slides_png(&content, w, &initial_theme, prefix, cli.slides);
                }
            }
            "odp" | "odp+svg" | "odp+png" => {
                let out_file = cli.export_file.as_deref().unwrap_or_else(|| {
                    eprintln!("Error: --export-file is required with --export {}", fmt);
                    process::exit(1);
                });
                let w = if width > 0 { width } else { 120 };
                let kind = if fmt == "odp+png" {
                    export::OdpImageKind::Png
                } else {
                    export::OdpImageKind::Svg
                };
                if fmt != "odp+png" {
                    eprintln!(
                        "Note: SVG-based ODP may not open in PowerPoint. Use --export odp+png for PowerPoint compatibility."
                    );
                }
                if let Err(e) =
                    export::export_odp(&content, w, &initial_theme, out_file, cli.slides, kind)
                {
                    eprintln!("Error writing '{}': {}", out_file, e);
                    process::exit(1);
                }
            }
            _ => {
                eprintln!(
                    "Unknown export format '{}'. Supported: html, svg, png, odp, odp+svg, odp+png",
                    fmt
                );
                process::exit(1);
            }
        }
        return;
    }

    // --export-prefix / --export-file without --export is an error
    if cli.export_prefix.is_some() {
        eprintln!("Error: --export-prefix requires --export svg or --export png");
        process::exit(1);
    }
    if cli.export_file.is_some() {
        eprintln!("Error: --export-file requires --export odp / odp+svg / odp+png");
        process::exit(1);
    }

    // Interactive or piped
    if io::stdout().is_terminal() && !cli.no_color {
        let opts = viewer::ViewerOptions {
            files: cli.files,
            initial_content: content,
            filename,
            theme: initial_theme,
            slide_mode: cli.slides,
            line_numbers,
            width_override: if width > 0 { Some(width) } else { None },
        };
        if let Err(e) = viewer::run(opts) {
            eprintln!("Viewer error: {}", e);
            process::exit(1);
        }
    } else {
        let w = if width > 0 {
            width
        } else {
            crossterm::terminal::size()
                .map(|(c, _)| c as usize)
                .unwrap_or(80)
        };
        let (lines, _) = if is_json {
            match json::render(&content, w, &initial_theme) {
                Ok(result) => result,
                Err(e) => {
                    eprintln!("JSON parse error: {}", e);
                    process::exit(1);
                }
            }
        } else {
            markdown::render(&content, w, &initial_theme, line_numbers)
        };
        let wrapped = style::wrap_lines(&lines, w);
        if cli.no_color {
            viewer::print_lines_plain(&wrapped);
        } else {
            viewer::print_lines(&wrapped);
        }
    }
}
