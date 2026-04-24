use std::io::{self, Write};

use crossterm::style::Color;
use unicode_width::UnicodeWidthStr;

use crate::markdown;
use crate::style::{LineMeta, wrap_lines};
use crate::theme::Theme;

pub fn to_html(content: &str, width: usize, theme: &Theme, filename: &str) {
    let (lines, _) = if filename.ends_with(".json") {
        match crate::json::render(content, width, theme) {
            Ok(result) => result,
            Err(_) => markdown::render(content, width, theme, false),
        }
    } else {
        markdown::render(content, width, theme, false)
    };
    let wrapped = wrap_lines(&lines, width);

    let mut out = io::stdout();
    let _ = writeln!(out, "<!DOCTYPE html>");
    let _ = writeln!(out, "<html><head>");
    let _ = writeln!(out, "<meta charset='utf-8'>");
    let _ = writeln!(
        out,
        "<style>body {{ font-family: 'SF Mono','Menlo','Consolas',monospace; background:{}; color:{}; padding:2em; line-height:1.4; }} pre {{ margin:0; }} .line {{ white-space:pre; min-height:1.2em; }}</style>",
        color_css(theme.bg),
        color_css(theme.fg)
    );
    let _ = writeln!(out, "</head><body>");

    for line in &wrapped {
        // Handle image placeholder lines
        if let LineMeta::Image {
            ref url,
            ref alt,
            row,
            ..
        } = line.meta
        {
            if row == 0 {
                if is_safe_img_src(url) {
                    let _ = writeln!(
                        out,
                        "<div class='line'><img src='{}' alt='{}' style='max-width:100%;height:auto;'></div>",
                        html_escape(url),
                        html_escape(alt)
                    );
                } else {
                    let _ = writeln!(out, "<div class='line'>{}</div>", html_escape(alt));
                }
            }
            continue;
        }

        let _ = write!(out, "<div class='line'>");
        if line.spans.is_empty() {
            let _ = write!(out, "&nbsp;");
        }
        for span in &line.spans {
            let mut styles = Vec::new();
            if let Some(fg) = span.style.fg {
                styles.push(format!("color:{}", color_css(fg)));
            }
            if let Some(bg) = span.style.bg {
                styles.push(format!("background:{}", color_css(bg)));
            }
            if span.style.bold {
                styles.push("font-weight:bold".into());
            }
            if span.style.italic {
                styles.push("font-style:italic".into());
            }
            match (span.style.underline, span.style.strikethrough) {
                (true, true) => {
                    styles.push("text-decoration:underline line-through".into());
                }
                (true, false) => {
                    styles.push("text-decoration:underline".into());
                }
                (false, true) => {
                    styles.push("text-decoration:line-through".into());
                }
                _ => {}
            }
            if span.style.dim {
                styles.push("opacity:0.5".into());
            }

            let text = html_escape(&span.text);

            if styles.is_empty() {
                let _ = write!(out, "{}", text);
            } else {
                let _ = write!(out, "<span style='{}'>", styles.join(";"));
                if let Some(ref url) = span.style.link_url {
                    if is_safe_url(url) {
                        let _ = write!(
                            out,
                            "<a href='{}' style='color:inherit;text-decoration:inherit'>{}</a>",
                            html_escape(url),
                            text
                        );
                    } else {
                        let _ = write!(out, "{}", text);
                    }
                } else {
                    let _ = write!(out, "{}", text);
                }
                let _ = write!(out, "</span>");
            }
        }
        let _ = writeln!(out, "</div>");
    }

    let _ = writeln!(out, "</body></html>");
}

fn color_css(c: Color) -> String {
    match c {
        Color::Rgb { r, g, b } => format!("#{:02x}{:02x}{:02x}", r, g, b),
        _ => "#000".into(),
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Strip all ASCII control characters (0x00–0x1F, 0x7F) that browsers silently
/// ignore when parsing URL schemes, which could bypass scheme checks.
/// Tabs, newlines, and carriage returns are also stripped because the URL
/// standard removes them before scheme matching.
fn strip_control_chars(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).collect()
}

/// Returns true if the URL scheme is safe for use in `<a href>`.
fn is_safe_url(url: &str) -> bool {
    let cleaned = strip_control_chars(url);
    let trimmed = cleaned.trim();
    let lower = trimmed.to_lowercase();
    // Allow common safe schemes, anchors, and relative paths
    if lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("mailto:")
        || trimmed.starts_with('#')
    {
        return true;
    }
    // Block known dangerous schemes
    if lower.starts_with("javascript:")
        || lower.starts_with("vbscript:")
        || lower.starts_with("data:")
    {
        return false;
    }
    // Allow relative paths (no colon before first slash)
    !lower.split('/').next().unwrap_or("").contains(':')
}

/// Returns true if the URL is safe for use in `<img src>`.
fn is_safe_img_src(url: &str) -> bool {
    let cleaned = strip_control_chars(url);
    let trimmed = cleaned.trim();
    let lower = trimmed.to_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return true;
    }
    // Allow only specific raster image data URIs (MIME must be followed by `;` or `,`)
    let safe_data_prefixes = [
        "data:image/png",
        "data:image/jpeg",
        "data:image/gif",
        "data:image/webp",
        "data:image/bmp",
    ];
    for prefix in &safe_data_prefixes {
        if let Some(rest) = lower.strip_prefix(prefix)
            && (rest.starts_with(';') || rest.starts_with(','))
        {
            return true;
        }
    }
    // Block dangerous schemes
    if lower.starts_with("javascript:")
        || lower.starts_with("vbscript:")
        || lower.starts_with("data:")
    {
        return false;
    }
    // Allow relative paths
    !lower.split('/').next().unwrap_or("").contains(':')
}

// ── SVG / PNG export ────────────────────────────────────────────────────────

/// Character cell metrics used when building the SVG viewport (3× scale).
const CHAR_WIDTH: f64 = 28.8;
const LINE_HEIGHT: f64 = 60.0;
const FONT_SIZE: f64 = 48.0;
const PAD: f64 = 60.0;

/// Render a slice of `Line`s to a self-contained SVG string.
/// Each `StyledSpan` becomes a `<tspan>` with the correct colours and
/// font attributes; background colours get a `<rect>` drawn behind the text.
pub fn to_svg_string(lines: &[crate::style::Line], width: usize, theme: &Theme) -> String {
    use std::fmt::Write as FmtWrite;

    let svg_w = width as f64 * CHAR_WIDTH + PAD * 2.0;
    let svg_h = lines.len() as f64 * LINE_HEIGHT + PAD * 2.0;

    let mut svg = String::new();
    let _ = write!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">"#,
        w = svg_w as u32,
        h = svg_h as u32,
    );

    // Background — use the theme's background colour so dark/light themes
    // both render correctly.
    let _ = write!(
        svg,
        r#"<rect width="{w}" height="{h}" fill="{bg}"/>"#,
        w = svg_w as u32,
        h = svg_h as u32,
        bg = color_css(theme.bg),
    );

    // Font definition embedded once; DejaVu Sans Mono is the primary fallback
    // for environments where Courier New is not installed.
    let _ = write!(
        svg,
        r#"<style>text{{font-family:'Courier New','DejaVu Sans Mono',Courier,monospace;font-size:{fs}px;dominant-baseline:auto;}}</style>"#,
        fs = FONT_SIZE as u32,
    );

    for (row, line) in lines.iter().enumerate() {
        let y_top = PAD + row as f64 * LINE_HEIGHT;
        let y_baseline = y_top + LINE_HEIGHT * 0.78; // ~78% from top = cap-height baseline

        // Background rects for spans that have a bg colour
        let mut col: f64 = 0.0;
        for span in &line.spans {
            let span_w = UnicodeWidthStr::width(span.text.as_str()) as f64 * CHAR_WIDTH;
            if let Some(bg) = span.style.bg {
                let _ = write!(
                    svg,
                    r#"<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{fill}"/>"#,
                    x = (PAD + col) as u32,
                    y = y_top as u32,
                    w = span_w as u32,
                    h = LINE_HEIGHT as u32,
                    fill = color_css(bg),
                );
            }
            col += span_w;
        }

        if line.spans.is_empty() {
            continue;
        }

        // Box-drawing lines (table/code borders) are rendered as SVG <rect>s
        // rather than font glyphs so they tile with zero gaps regardless of font.
        if is_box_drawing_line(line) {
            let bar_t = (LINE_HEIGHT * 0.07).max(2.0);
            let mid_y = y_top + LINE_HEIGHT * 0.5;
            let color = line
                .spans
                .iter()
                .find_map(|s| s.style.fg)
                .unwrap_or(theme.fg);
            let fill = color_css(color);
            let mut cx = PAD;
            for span in &line.spans {
                for ch in span.text.chars() {
                    let (hl, hr, vu, vd) = box_char_dirs(ch);
                    let char_mid_x = cx + CHAR_WIDTH * 0.5;
                    // horizontal segments
                    if hl {
                        let _ = write!(svg,
                            r#"<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{fill}"/>"#,
                            x = cx as u32,
                            y = (mid_y - bar_t / 2.0) as u32,
                            w = (CHAR_WIDTH / 2.0).ceil() as u32,
                            h = bar_t.ceil() as u32,
                            fill = fill);
                    }
                    if hr {
                        let _ = write!(svg,
                            r#"<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{fill}"/>"#,
                            x = (cx + CHAR_WIDTH / 2.0) as u32,
                            y = (mid_y - bar_t / 2.0) as u32,
                            w = (CHAR_WIDTH / 2.0).ceil() as u32,
                            h = bar_t.ceil() as u32,
                            fill = fill);
                    }
                    // vertical segments
                    if vu {
                        let _ = write!(svg,
                            r#"<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{fill}"/>"#,
                            x = (char_mid_x - bar_t / 2.0) as u32,
                            y = y_top as u32,
                            w = bar_t.ceil() as u32,
                            h = (LINE_HEIGHT / 2.0).ceil() as u32,
                            fill = fill);
                    }
                    if vd {
                        let _ = write!(svg,
                            r#"<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{fill}"/>"#,
                            x = (char_mid_x - bar_t / 2.0) as u32,
                            y = (y_top + LINE_HEIGHT / 2.0) as u32,
                            w = bar_t.ceil() as u32,
                            h = (LINE_HEIGHT / 2.0).ceil() as u32,
                            fill = fill);
                    }
                    cx += CHAR_WIDTH;
                }
            }
            continue;
        }

        // Text + tspans — each tspan gets an explicit x so wide/emoji chars
        // (which fall back to a non-monospace font) cannot shift subsequent spans.
        let _ = write!(
            svg,
            r#"<text y="{y}" xml:space="preserve">"#,
            y = y_baseline as u32,
        );

        let mut x_pos = PAD;
        for span in &line.spans {
            let span_w = UnicodeWidthStr::width(span.text.as_str()) as f64 * CHAR_WIDTH;
            let mut attrs = String::new();
            let _ = write!(attrs, r#" x="{}""#, x_pos as u32);
            let fg = span.style.fg.unwrap_or(theme.fg);
            let _ = write!(attrs, r#" fill="{}""#, color_css(fg));
            if span.style.bold {
                let _ = write!(attrs, r#" font-weight="bold""#);
            }
            if span.style.italic {
                let _ = write!(attrs, r#" font-style="italic""#);
            }
            let mut decorations: Vec<&str> = Vec::new();
            if span.style.underline {
                decorations.push("underline");
            }
            if span.style.strikethrough {
                decorations.push("line-through");
            }
            if !decorations.is_empty() {
                let _ = write!(attrs, r#" text-decoration="{}""#, decorations.join(" "));
            }
            if span.style.dim {
                let _ = write!(attrs, r#" opacity="0.5""#);
            }
            let _ = write!(svg, "<tspan{}>", attrs);
            svg_escape_into(&mut svg, &span.text);
            let _ = write!(svg, "</tspan>");
            x_pos += span_w;
        }

        let _ = write!(svg, "</text>");
    }

    let _ = write!(svg, "</svg>");
    svg
}

/// Returns true if every non-space character in the line is a box-drawing glyph.
/// Used by `to_svg_string` to switch to rect-based rendering for table borders.
fn is_box_drawing_line(line: &crate::style::Line) -> bool {
    line.spans.iter().all(|s| s.text.chars().all(|c| matches!(c, ' ' | '─' | '│' | '╭' | '╮' | '╰' | '╯' | '├' | '┤' | '┬' | '┴' | '┼')))
        && line.spans.iter().any(|s| s.text.chars().any(|c| c != ' '))
}

/// Returns which sides of a cell a box-drawing char connects to: (left, right, up, down).
fn box_char_dirs(c: char) -> (bool, bool, bool, bool) {
    match c {
        '─' => (true,  true,  false, false),
        '│' => (false, false, true,  true ),
        '╭' => (false, true,  false, true ),  // arc: down + right
        '╮' => (true,  false, false, true ),  // arc: down + left
        '╰' => (false, true,  true,  false),  // arc: up   + right
        '╯' => (true,  false, true,  false),  // arc: up   + left
        '├' => (false, true,  true,  true ),
        '┤' => (true,  false, true,  true ),
        '┬' => (true,  true,  false, true ),
        '┴' => (true,  true,  true,  false),
        '┼' => (true,  true,  true,  true ),
        _   => (false, false, false, false),
    }
}

/// Escape text content for safe embedding inside SVG.
fn svg_escape_into(out: &mut String, s: &str) {
    use std::fmt::Write as FmtWrite;
    for c in s.chars() {
        match c {
            '&' => {
                let _ = out.write_str("&amp;");
            }
            '<' => {
                let _ = out.write_str("&lt;");
            }
            '>' => {
                let _ = out.write_str("&gt;");
            }
            '"' => {
                let _ = out.write_str("&quot;");
            }
            '\'' => {
                let _ = out.write_str("&#39;");
            }
            c => {
                let _ = out.write_char(c);
            }
        }
    }
}

/// Returns `(slide_start, slide_end)` index pairs into `wrapped` for each
/// slide. If there are no `SlideBreak` lines the whole document is one slide.
fn slide_ranges(wrapped: &[crate::style::Line]) -> Vec<(usize, usize)> {
    let mut starts: Vec<usize> = vec![0];
    for (i, line) in wrapped.iter().enumerate() {
        if matches!(line.meta, LineMeta::SlideBreak) {
            starts.push(i + 1);
        }
    }
    let total = wrapped.len();
    starts
        .iter()
        .enumerate()
        .map(|(idx, &start)| {
            let end = starts.get(idx + 1).copied().unwrap_or(total);
            (start, end)
        })
        .collect()
}

/// Export the document (or its slides) as a series of SVG files.
/// With `slide_mode = true`, one file per slide; otherwise a single file.
/// Files are written as `{prefix}0001.svg`, `{prefix}0002.svg`, …
pub fn export_slides_svg(
    content: &str,
    width: usize,
    theme: &Theme,
    prefix: &str,
    slide_mode: bool,
) {
    use std::fs;
    use std::path::Path;

    let (lines, _) = markdown::render(content, width, theme, false);
    let wrapped = crate::style::wrap_lines(&lines, width);

    let ranges = if slide_mode {
        slide_ranges(&wrapped)
    } else {
        vec![(0, wrapped.len())]
    };

    // Ensure output directory exists
    if let Some(parent) = Path::new(prefix).parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(e) = fs::create_dir_all(parent) {
                eprintln!("Error creating output directory: {}", e);
                std::process::exit(1);
            }
        }
    }

    for (idx, (start, end)) in ranges.iter().enumerate() {
        let slide_lines = &wrapped[*start..*end];
        let svg = to_svg_string(slide_lines, width, theme);
        let path = format!("{}{:04}.svg", prefix, idx + 1);
        if let Err(e) = fs::write(&path, &svg) {
            eprintln!("Error writing '{}': {}", path, e);
            std::process::exit(1);
        }
        eprintln!("Wrote {}", path);
    }
}

/// Rasterize an SVG string to a PNG byte vector using `resvg`.
/// `bg` is the theme background colour used to pre-fill the pixmap so that
/// image viewers that don't handle alpha see the correct opaque background.
fn svg_to_png(svg_str: &str, bg: Color) -> Vec<u8> {
    use resvg::tiny_skia;
    use resvg::usvg;

    let mut opt = usvg::Options::default();
    // Load system fonts so that text elements are rendered correctly.
    opt.fontdb_mut().load_system_fonts();
    let tree = match usvg::Tree::from_str(svg_str, &opt) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("SVG parse error: {}", e);
            std::process::exit(1);
        }
    };

    let size = tree.size();
    let w = size.width().ceil() as u32;
    let h = size.height().ceil() as u32;

    let mut pixmap = match tiny_skia::Pixmap::new(w, h) {
        Some(p) => p,
        None => {
            eprintln!("Error: could not allocate {}×{} pixmap", w, h);
            std::process::exit(1);
        }
    };

    // Pre-fill with the theme background so the PNG is fully opaque even if
    // the SVG rect blending leaves residual transparency in the alpha channel.
    let (r, g, b) = match bg {
        Color::Rgb { r, g, b } => (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0),
        _ => (0.0, 0.0, 0.0),
    };
    pixmap.fill(tiny_skia::Color::from_rgba(r, g, b, 1.0).unwrap_or(tiny_skia::Color::BLACK));

    resvg::render(
        &tree,
        tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );

    pixmap.encode_png().unwrap_or_else(|e| {
        eprintln!("PNG encoding error: {}", e);
        std::process::exit(1);
    })
}

/// Rasterize an SVG string to a PNG at exactly `target_w × target_h` pixels.
/// The SVG is scaled to fit within the target dimensions maintaining aspect
/// ratio (letterboxed), with `bg` filling any unused area.
fn svg_to_png_sized(svg_str: &str, bg: Color, target_w: u32, target_h: u32) -> Vec<u8> {
    use resvg::tiny_skia;
    use resvg::usvg;

    let mut opt = usvg::Options::default();
    opt.fontdb_mut().load_system_fonts();
    let tree = match usvg::Tree::from_str(svg_str, &opt) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("SVG parse error: {}", e);
            std::process::exit(1);
        }
    };

    let mut pixmap = match tiny_skia::Pixmap::new(target_w, target_h) {
        Some(p) => p,
        None => {
            eprintln!("Error: could not allocate {}×{} pixmap", target_w, target_h);
            std::process::exit(1);
        }
    };

    let (r, g, b) = match bg {
        Color::Rgb { r, g, b } => (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0),
        _ => (0.0, 0.0, 0.0),
    };
    pixmap.fill(tiny_skia::Color::from_rgba(r, g, b, 1.0).unwrap_or(tiny_skia::Color::BLACK));

    // Scale to fit, maintaining aspect ratio (letterbox).
    let svg_w = tree.size().width();
    let svg_h = tree.size().height();
    let scale = (target_w as f32 / svg_w).min(target_h as f32 / svg_h);
    let tx = (target_w as f32 - svg_w * scale) / 2.0;
    let ty = (target_h as f32 - svg_h * scale) / 2.0;
    let transform = tiny_skia::Transform::from_row(scale, 0.0, 0.0, scale, tx, ty);

    resvg::render(&tree, transform, &mut pixmap.as_mut());

    pixmap.encode_png().unwrap_or_else(|e| {
        eprintln!("PNG encoding error: {}", e);
        std::process::exit(1);
    })
}

/// Export the document (or its slides) as a series of PNG files.
/// Files are written as `{prefix}0001.png`, `{prefix}0002.png`, …
pub fn export_slides_png(
    content: &str,
    width: usize,
    theme: &Theme,
    prefix: &str,
    slide_mode: bool,
) {
    use std::fs;
    use std::path::Path;

    let (lines, _) = markdown::render(content, width, theme, false);
    let wrapped = crate::style::wrap_lines(&lines, width);

    let ranges = if slide_mode {
        slide_ranges(&wrapped)
    } else {
        vec![(0, wrapped.len())]
    };

    // Ensure output directory exists
    if let Some(parent) = Path::new(prefix).parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(e) = fs::create_dir_all(parent) {
                eprintln!("Error creating output directory: {}", e);
                std::process::exit(1);
            }
        }
    }

    for (idx, (start, end)) in ranges.iter().enumerate() {
        let slide_lines = &wrapped[*start..*end];
        let svg = to_svg_string(slide_lines, width, theme);
        let png = svg_to_png(&svg, theme.bg);
        let path = format!("{}{:04}.png", prefix, idx + 1);
        if let Err(e) = fs::write(&path, &png) {
            eprintln!("Error writing '{}': {}", path, e);
            std::process::exit(1);
        }
        eprintln!("Wrote {}", path);
    }
}

// ── ODP export ───────────────────────────────────────────────────────────────

/// Which image format to embed inside the ODP archive.
pub enum OdpImageKind {
    /// Embed SVG (default — smaller, vector, renders perfectly in LibreOffice).
    Svg,
    /// Embed rasterised PNG (1920×1080, compatible with PowerPoint and any ODP viewer).
    Png,
}

/// Standard 16:9 widescreen presentation dimensions used for the ODP page layout.
const PRES_W_CM: f64 = 25.4; // 10 inches
const PRES_H_CM: f64 = 14.288; // 10 × 9/16 inches
const PRES_PNG_W: u32 = 3840;
const PRES_PNG_H: u32 = 2160;

/// Export the document (or its slides) as a single ODP file.
///
/// Each slide is rendered to an SVG/PNG in memory and stored in the ZIP as
/// `Pictures/slideNNNN.svg` / `.png`.  No temporary files are created.
/// The ODP page size is always the standard 16:9 widescreen (25.4 × 14.29 cm)
/// so the file opens correctly in LibreOffice, Nextcloud, and PowerPoint.
pub fn export_odp(
    content: &str,
    width: usize,
    theme: &Theme,
    out_path: &str,
    slide_mode: bool,
    kind: OdpImageKind,
) -> std::io::Result<()> {
    use std::fs::File;
    use std::io::{BufWriter, Write};
    use zip::CompressionMethod;
    use zip::write::SimpleFileOptions;

    // ── 1. Render markdown → slides ──────────────────────────────────────
    let (lines, _) = markdown::render(content, width, theme, false);
    let wrapped = crate::style::wrap_lines(&lines, width);
    let ranges = if slide_mode {
        slide_ranges(&wrapped)
    } else {
        vec![(0, wrapped.len())]
    };

    // ── 2. Build per-slide image bytes (in memory) ───────────────────────
    let ext = match kind {
        OdpImageKind::Svg => "svg",
        OdpImageKind::Png => "png",
    };

    // Compute the tallest slide so every slide SVG has the same height,
    // matching the fixed page dimensions exactly (no stretching in viewers).
    let max_lines = ranges.iter().map(|(s, e)| e - s).max().unwrap_or(1).max(1);

    let mut slide_images: Vec<Vec<u8>> = Vec::with_capacity(ranges.len());
    for (start, end) in &ranges {
        // Exclude trailing SlideBreak lines from the rendered image.
        let raw = &wrapped[*start..*end];
        let trimmed_end = raw
            .iter()
            .rposition(|l| !matches!(l.meta, crate::style::LineMeta::SlideBreak))
            .map(|i| i + 1)
            .unwrap_or(0);
        let slide_lines = &raw[..trimmed_end];

        // Pad with empty lines so every slide SVG has exactly max_lines rows,
        // ensuring the SVG dimensions match the fixed page layout.
        let padding = max_lines.saturating_sub(slide_lines.len());
        let mut padded: Vec<crate::style::Line>;
        let lines_for_svg: &[crate::style::Line] = if padding > 0 {
            padded = slide_lines.to_vec();
            padded.extend((0..padding).map(|_| crate::style::Line::empty()));
            &padded
        } else {
            slide_lines
        };

        let svg = to_svg_string(lines_for_svg, width, theme);
        let bytes: Vec<u8> = match kind {
            OdpImageKind::Svg => svg.into_bytes(),
            OdpImageKind::Png => svg_to_png_sized(&svg, theme.bg, PRES_PNG_W, PRES_PNG_H),
        };
        slide_images.push(bytes);
    }

    // ── 3. Build the ZIP (ODP archive) directly to file ──────────────────
    if let Some(parent) = std::path::Path::new(out_path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let file = File::create(out_path)?;
    let writer = BufWriter::new(file);
    let mut zip = zip::ZipWriter::new(writer);

    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    // mimetype MUST be the first entry and STORED (uncompressed) per ODF spec.
    zip.start_file("mimetype", stored)?;
    zip.write_all(b"application/vnd.oasis.opendocument.presentation")?;

    // META-INF/manifest.xml
    zip.start_file("META-INF/manifest.xml", deflated)?;
    zip.write_all(build_manifest(ext, ranges.len()).as_bytes())?;

    // styles.xml — page layout + master page (required by PowerPoint's ODP importer)
    zip.start_file("styles.xml", deflated)?;
    zip.write_all(build_styles_xml(PRES_W_CM, PRES_H_CM).as_bytes())?;

    // content.xml — one <draw:page> per slide
    zip.start_file("content.xml", deflated)?;
    zip.write_all(build_content_xml(ext, ranges.len(), PRES_W_CM, PRES_H_CM).as_bytes())?;

    // Pictures/slideNNNN.{svg|png}
    for (idx, bytes) in slide_images.iter().enumerate() {
        let name = format!("Pictures/slide{:04}.{}", idx + 1, ext);
        zip.start_file(&name, deflated)?;
        zip.write_all(bytes)?;
    }

    zip.finish()?;

    eprintln!("Wrote {} ({} slide(s))", out_path, ranges.len());
    Ok(())
}

/// Build the ODF manifest listing all entries in the archive.
fn build_manifest(ext: &str, n_slides: usize) -> String {
    use std::fmt::Write as FmtWrite;
    let mut s = String::new();
    let _ = writeln!(
        s,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.3">
 <manifest:file-entry manifest:full-path="/" manifest:media-type="application/vnd.oasis.opendocument.presentation"/>
 <manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/>
 <manifest:file-entry manifest:full-path="styles.xml" manifest:media-type="text/xml"/>"#
    );
    let mime = if ext == "svg" {
        "image/svg+xml"
    } else {
        "image/png"
    };
    for i in 1..=n_slides {
        let _ = writeln!(
            s,
            r#" <manifest:file-entry manifest:full-path="Pictures/slide{:04}.{}" manifest:media-type="{}"/>"#,
            i, ext, mime
        );
    }
    let _ = writeln!(s, "</manifest:manifest>");
    s
}

/// Build styles.xml containing the page layout and master page definition.
/// PowerPoint's ODP importer requires master-page to live in styles.xml,
/// not content.xml.
fn build_styles_xml(w_cm: f64, h_cm: f64) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-styles
  xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
  xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0"
  xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0"
  xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0"
  xmlns:presentation="urn:oasis:names:tc:opendocument:xmlns:presentation:1.0"
  office:version="1.3">
 <office:styles/>
 <office:automatic-styles>
  <style:page-layout style:name="pl1">
   <style:page-layout-properties fo:page-width="{w:.3}cm" fo:page-height="{h:.3}cm" style:print-orientation="landscape"/>
  </style:page-layout>
  <style:style style:name="dp1" style:family="drawing-page"/>
 </office:automatic-styles>
 <office:master-styles>
  <style:master-page style:name="Default" style:page-layout-name="pl1" draw:style-name="dp1"/>
 </office:master-styles>
</office:document-styles>
"#,
        w = w_cm,
        h = h_cm,
    )
}

/// Build the ODF content.xml with one `<draw:page>` per slide.
fn build_content_xml(ext: &str, n_slides: usize, w_cm: f64, h_cm: f64) -> String {
    use std::fmt::Write as FmtWrite;

    let wstr = format!("{:.3}cm", w_cm);
    let hstr = format!("{:.3}cm", h_cm);

    let mut s = String::new();

    let _ = write!(
        s,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content
  xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
  xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0"
  xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
  xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0"
  xmlns:presentation="urn:oasis:names:tc:opendocument:xmlns:presentation:1.0"
  xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0"
  xmlns:xlink="http://www.w3.org/1999/xlink"
  xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0"
  office:version="1.3">
 <office:automatic-styles>
  <style:style style:name="gr1" style:family="graphic">
   <style:graphic-properties style:protect="position size"/>
  </style:style>
 </office:automatic-styles>
 <office:body>
  <office:presentation>
"#
    );

    for i in 1..=n_slides {
        let href = format!("Pictures/slide{:04}.{}", i, ext);
        let _ = write!(
            s,
            r#"   <draw:page draw:name="Slide{i}" draw:style-name="dp1" draw:master-page-name="Default">
    <draw:frame draw:style-name="gr1" svg:x="0cm" svg:y="0cm" svg:width="{w}" svg:height="{h}">
     <draw:image xlink:href="{href}" xlink:type="simple" xlink:show="embed" xlink:actuate="onLoad"/>
    </draw:frame>
   </draw:page>
"#,
            i = i,
            w = wstr,
            h = hstr,
            href = href,
        );
    }

    let _ = write!(
        s,
        r#"  </office:presentation>
 </office:body>
</office:document-content>
"#
    );
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── is_safe_url ─────────────────────────────────────────────────────

    #[test]
    fn safe_url_allows_http() {
        assert!(is_safe_url("http://example.com"));
        assert!(is_safe_url("https://example.com/page"));
    }

    #[test]
    fn safe_url_allows_mailto() {
        assert!(is_safe_url("mailto:user@example.com"));
    }

    #[test]
    fn safe_url_allows_anchor() {
        assert!(is_safe_url("#section-1"));
    }

    #[test]
    fn safe_url_allows_relative_paths() {
        assert!(is_safe_url("./foo/bar.html"));
        assert!(is_safe_url("images/photo.png"));
        assert!(is_safe_url("../other.md"));
    }

    #[test]
    fn safe_url_blocks_javascript() {
        assert!(!is_safe_url("javascript:alert(1)"));
        assert!(!is_safe_url("JAVASCRIPT:alert(1)"));
        assert!(!is_safe_url("JavaScript:void(0)"));
    }

    #[test]
    fn safe_url_blocks_vbscript() {
        assert!(!is_safe_url("vbscript:exec"));
        assert!(!is_safe_url("VBSCRIPT:MsgBox"));
    }

    #[test]
    fn safe_url_blocks_data() {
        assert!(!is_safe_url("data:text/html,<script>alert(1)</script>"));
    }

    #[test]
    fn safe_url_blocks_control_char_bypass() {
        assert!(!is_safe_url("java\x01script:alert(1)"));
        assert!(!is_safe_url("java\x0Bscript:alert(1)"));
        assert!(!is_safe_url("\x00javascript:alert(1)"));
    }

    #[test]
    fn safe_url_blocks_tab_newline_in_scheme() {
        // Browsers strip tabs and newlines before scheme matching (URL standard),
        // so these must be stripped before our checks too.
        assert!(!is_safe_url("java\tscript:alert(1)"));
        assert!(!is_safe_url("java\nscript:alert(1)"));
        assert!(!is_safe_url("java\rscript:alert(1)"));
        assert!(!is_safe_url("j\ta\nv\ra\tscript:alert(1)"));
    }

    #[test]
    fn safe_url_handles_whitespace() {
        assert!(is_safe_url("  https://example.com  "));
        assert!(!is_safe_url("  javascript:alert(1)  "));
    }

    #[test]
    fn safe_url_handles_empty() {
        // Empty/whitespace-only: no colon before slash → treated as relative
        assert!(is_safe_url(""));
        assert!(is_safe_url("   "));
    }

    // ── is_safe_img_src ─────────────────────────────────────────────────

    #[test]
    fn safe_img_allows_http() {
        assert!(is_safe_img_src("http://example.com/img.png"));
        assert!(is_safe_img_src("https://cdn.example.com/photo.jpg"));
    }

    #[test]
    fn safe_img_allows_data_image() {
        assert!(is_safe_img_src("data:image/png;base64,iVBOR..."));
        assert!(is_safe_img_src("data:image/png,rawdata"));
        assert!(is_safe_img_src("data:image/jpeg;base64,/9j/4..."));
    }

    #[test]
    fn safe_img_blocks_data_image_prefix_spoof() {
        // "data:image/pnganything" should not match — MIME must be followed by ; or ,
        assert!(!is_safe_img_src("data:image/pngevil"));
        assert!(!is_safe_img_src("data:image/jpegscript:alert(1)"));
    }

    #[test]
    fn safe_img_blocks_data_non_image() {
        assert!(!is_safe_img_src("data:text/html,<script>alert(1)</script>"));
        assert!(!is_safe_img_src("data:application/pdf,stuff"));
    }

    #[test]
    fn safe_img_blocks_svg_xss() {
        assert!(!is_safe_img_src(
            "data:image/svg+xml,<svg onload='alert(1)'>"
        ));
        assert!(!is_safe_img_src(
            "data:image/svg+xml;base64,PHN2ZyBvbmxvYWQ9ImFsZXJ0KDEpIj4="
        ));
    }

    #[test]
    fn safe_img_blocks_javascript() {
        assert!(!is_safe_img_src("javascript:alert(1)"));
        assert!(!is_safe_img_src("JAVASCRIPT:alert(1)"));
    }

    #[test]
    fn safe_img_blocks_vbscript() {
        assert!(!is_safe_img_src("vbscript:exec"));
    }

    #[test]
    fn safe_img_allows_relative_paths() {
        assert!(is_safe_img_src("./images/photo.png"));
        assert!(is_safe_img_src("photo.jpg"));
    }

    #[test]
    fn safe_img_blocks_control_char_bypass() {
        assert!(!is_safe_img_src("java\x01script:alert(1)"));
        assert!(!is_safe_img_src("\x00javascript:alert(1)"));
    }

    #[test]
    fn safe_img_handles_empty() {
        assert!(is_safe_img_src(""));
        assert!(is_safe_img_src("   "));
    }
}
