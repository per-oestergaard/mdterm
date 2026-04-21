/// Integration tests that exercise every export format (HTML, SVG, PNG) in
/// both dark and light themes.  All output files are written under `temp/`.

use std::fs;
use std::path::Path;
use std::process::Command;

/// Path to the test Markdown fixture used as input.
const INPUT_MD: &str = "test.md";

/// Markdown fixture with explicit slide breaks (---) for slide-mode tests.
const SLIDES_MD: &str = "tests/fixtures/slides.md";

/// Expected number of slides in SLIDES_MD (three `---` separators → 3 slides).
const EXPECTED_SLIDES: usize = 3;

/// Returns the path to the compiled debug binary.
fn binary() -> String {
    let mut path = std::env::current_exe()
        .expect("could not determine test binary path")
        .parent()
        .expect("no parent dir")
        .to_path_buf();

    // Walk up from the test binary location to find the workspace target/debug.
    // typical layout: target/debug/deps/<test-binary>
    for _ in 0..3 {
        let candidate = path.join("mdterm");
        if candidate.exists() {
            return candidate.to_string_lossy().into_owned();
        }
        path = path.parent().unwrap_or(&path).to_path_buf();
    }
    // Fallback: let PATH find it (e.g. after `cargo install`)
    "mdterm".into()
}

fn ensure_temp_dir() {
    fs::create_dir_all("temp").expect("could not create temp/ directory");
}

/// Collect all files under the directory of `prefix` whose names start with
/// the file-stem part of `prefix` and end with `ext`, sorted alphabetically.
fn collect_files(prefix: &str, ext: &str) -> Vec<String> {
    let dir = Path::new(prefix).parent().unwrap_or(Path::new("."));
    let stem = Path::new(prefix)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();

    let mut files: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with(&stem) && name.ends_with(ext) {
                Some(e.path().to_string_lossy().into_owned())
            } else {
                None
            }
        })
        .collect();
    files.sort();
    files
}

// ── HTML ────────────────────────────────────────────────────────────────────

fn export_html(theme: &str, out_path: &str) {
    let output = Command::new(binary())
        .args(["--theme", theme, "--export", "html", INPUT_MD])
        .output()
        .unwrap_or_else(|e| panic!("failed to run mdterm for html/{theme}: {e}"));

    assert!(
        output.status.success(),
        "mdterm html/{theme} exited with {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output.stdout.is_empty(),
        "html/{theme} output was empty"
    );

    fs::write(out_path, &output.stdout)
        .unwrap_or_else(|e| panic!("could not write {out_path}: {e}"));

    // Basic sanity: the file should look like an HTML document.
    let html = String::from_utf8_lossy(&output.stdout);
    assert!(html.contains("<!DOCTYPE html>"), "html/{theme} missing DOCTYPE");
    assert!(html.contains("</html>"), "html/{theme} missing closing tag");

    eprintln!("Wrote {out_path}  ({} bytes)", output.stdout.len());
}

#[test]
fn html_dark() {
    ensure_temp_dir();
    export_html("dark", "temp/dark.html");
}

#[test]
fn html_light() {
    ensure_temp_dir();
    export_html("light", "temp/light.html");
}

// ── SVG ─────────────────────────────────────────────────────────────────────

fn export_svg(theme: &str, prefix: &str) -> Vec<String> {
    let status = Command::new(binary())
        .args([
            "--theme", theme,
            "--export", "svg",
            "--export-prefix", prefix,
            INPUT_MD,
        ])
        .status()
        .unwrap_or_else(|e| panic!("failed to run mdterm for svg/{theme}: {e}"));

    assert!(
        status.success(),
        "mdterm svg/{theme} exited with {:?}",
        status.code()
    );

    collect_files(prefix, ".svg")
}

#[test]
fn svg_dark() {
    ensure_temp_dir();
    let files = export_svg("dark", "temp/dark_svg_");
    assert!(!files.is_empty(), "no SVG files written for dark theme");
    for f in &files {
        let content = fs::read_to_string(f).unwrap();
        assert!(content.starts_with("<svg "), "{f} is not a valid SVG");
        assert!(content.contains("<rect "), "{f} missing background rect");
        eprintln!("Wrote {f}  ({} bytes)", content.len());
    }
}

#[test]
fn svg_light() {
    ensure_temp_dir();
    let files = export_svg("light", "temp/light_svg_");
    assert!(!files.is_empty(), "no SVG files written for light theme");
    for f in &files {
        let content = fs::read_to_string(f).unwrap();
        assert!(content.starts_with("<svg "), "{f} is not a valid SVG");
        assert!(content.contains("<rect "), "{f} missing background rect");
        eprintln!("Wrote {f}  ({} bytes)", content.len());
    }
}

// ── PNG ─────────────────────────────────────────────────────────────────────

fn export_png(theme: &str, prefix: &str) -> Vec<String> {
    let status = Command::new(binary())
        .args([
            "--theme", theme,
            "--export", "png",
            "--export-prefix", prefix,
            INPUT_MD,
        ])
        .status()
        .unwrap_or_else(|e| panic!("failed to run mdterm for png/{theme}: {e}"));

    assert!(
        status.success(),
        "mdterm png/{theme} exited with {:?}",
        status.code()
    );

    collect_files(prefix, ".png")
}

/// PNG magic bytes: `\x89PNG\r\n\x1a\n`
const PNG_MAGIC: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

#[test]
fn png_dark() {
    ensure_temp_dir();
    let files = export_png("dark", "temp/dark_png_");
    assert!(!files.is_empty(), "no PNG files written for dark theme");
    for f in &files {
        let bytes = fs::read(f).unwrap();
        assert!(
            bytes.starts_with(PNG_MAGIC),
            "{f} does not have a PNG magic number"
        );
        assert!(bytes.len() > 1024, "{f} seems too small to contain text");
        eprintln!("Wrote {f}  ({} bytes)", bytes.len());
    }
}

#[test]
fn png_light() {
    ensure_temp_dir();
    let files = export_png("light", "temp/light_png_");
    assert!(!files.is_empty(), "no PNG files written for light theme");
    for f in &files {
        let bytes = fs::read(f).unwrap();
        assert!(
            bytes.starts_with(PNG_MAGIC),
            "{f} does not have a PNG magic number"
        );
        assert!(bytes.len() > 1024, "{f} seems too small to contain text");
        eprintln!("Wrote {f}  ({} bytes)", bytes.len());
    }
}

// ── Slide mode (SVG) ────────────────────────────────────────────────────────

fn export_svg_slides(theme: &str, prefix: &str) -> Vec<String> {
    let status = Command::new(binary())
        .args([
            "--theme", theme,
            "--slides",
            "--export", "svg",
            "--export-prefix", prefix,
            SLIDES_MD,
        ])
        .status()
        .unwrap_or_else(|e| panic!("failed to run mdterm for svg-slides/{theme}: {e}"));

    assert!(
        status.success(),
        "mdterm svg-slides/{theme} exited with {:?}",
        status.code()
    );

    collect_files(prefix, ".svg")
}

#[test]
fn svg_slides_dark() {
    ensure_temp_dir();
    let files = export_svg_slides("dark", "temp/dark_slides_svg_");
    assert_eq!(
        files.len(), EXPECTED_SLIDES,
        "expected {EXPECTED_SLIDES} SVG slide files for dark theme, got {}",
        files.len()
    );
    for f in &files {
        let content = fs::read_to_string(f).unwrap();
        assert!(content.starts_with("<svg "), "{f} is not a valid SVG");
        assert!(content.contains("<rect "), "{f} missing background rect");
        eprintln!("Wrote {f}  ({} bytes)", content.len());
    }
}

#[test]
fn svg_slides_light() {
    ensure_temp_dir();
    let files = export_svg_slides("light", "temp/light_slides_svg_");
    assert_eq!(
        files.len(), EXPECTED_SLIDES,
        "expected {EXPECTED_SLIDES} SVG slide files for light theme, got {}",
        files.len()
    );
    for f in &files {
        let content = fs::read_to_string(f).unwrap();
        assert!(content.starts_with("<svg "), "{f} is not a valid SVG");
        assert!(content.contains("<rect "), "{f} missing background rect");
        eprintln!("Wrote {f}  ({} bytes)", content.len());
    }
}

// ── Slide mode (PNG) ────────────────────────────────────────────────────────

fn export_png_slides(theme: &str, prefix: &str) -> Vec<String> {
    let status = Command::new(binary())
        .args([
            "--theme", theme,
            "--slides",
            "--export", "png",
            "--export-prefix", prefix,
            SLIDES_MD,
        ])
        .status()
        .unwrap_or_else(|e| panic!("failed to run mdterm for png-slides/{theme}: {e}"));

    assert!(
        status.success(),
        "mdterm png-slides/{theme} exited with {:?}",
        status.code()
    );

    collect_files(prefix, ".png")
}

#[test]
fn png_slides_dark() {
    ensure_temp_dir();
    let files = export_png_slides("dark", "temp/dark_slides_png_");
    assert_eq!(
        files.len(), EXPECTED_SLIDES,
        "expected {EXPECTED_SLIDES} PNG slide files for dark theme, got {}",
        files.len()
    );
    for f in &files {
        let bytes = fs::read(f).unwrap();
        assert!(
            bytes.starts_with(PNG_MAGIC),
            "{f} does not have a PNG magic number"
        );
        assert!(bytes.len() > 256, "{f} seems too small to contain content");
        eprintln!("Wrote {f}  ({} bytes)", bytes.len());
    }
}

#[test]
fn png_slides_light() {
    ensure_temp_dir();
    let files = export_png_slides("light", "temp/light_slides_png_");
    assert_eq!(
        files.len(), EXPECTED_SLIDES,
        "expected {EXPECTED_SLIDES} PNG slide files for light theme, got {}",
        files.len()
    );
    for f in &files {
        let bytes = fs::read(f).unwrap();
        assert!(
            bytes.starts_with(PNG_MAGIC),
            "{f} does not have a PNG magic number"
        );
        assert!(bytes.len() > 256, "{f} seems too small to contain content");
        eprintln!("Wrote {f}  ({} bytes)", bytes.len());
    }
}
