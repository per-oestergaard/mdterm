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

/// Strip C0 control characters before handing a string to the URL parser.
///
/// Several browsers silently ignore C0 controls (U+0000–U+001F) when matching
/// URL schemes, so `"java\x01script:"` would reach the DOM as `"javascript:"`.
/// The WHATWG URL standard strips only tabs and newlines; we strip all C0
/// controls to be safe against every browser quirk.
fn strip_url_controls(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).collect()
}

/// Returns true if the URL scheme is safe for use in `<a href>`.
///
/// Uses an **allowlist** — only `http`, `https`, `mailto`, anchors (`#…`),
/// and scheme-less relative paths are permitted.  Any scheme not on the list
/// (including `javascript:`, `vbscript:`, `data:`, `blob:`, `ws:`, …) is
/// blocked.  URL parsing is delegated to the `url` crate (WHATWG-compliant)
/// so there is no hand-rolled scheme extraction to get wrong.
fn is_safe_url(raw: &str) -> bool {
    use url::Url;
    let s = strip_url_controls(raw);
    let s = s.trim();
    if s.is_empty() || s.starts_with('#') {
        return true;
    }
    match Url::parse(s) {
        Ok(u) => matches!(u.scheme(), "http" | "https" | "mailto"),
        // Not an absolute URL — safe if there is no scheme-like prefix.
        Err(_) => !s.split('/').next().unwrap_or("").contains(':'),
    }
}

/// Returns true if the URL is safe for use in `<img src>`.
///
/// Allowlist: `http`/`https` and a small set of raster-only `data:` image
/// MIME types.  SVG data URIs are excluded because they can carry scripts.
/// Any unrecognised scheme is blocked.
fn is_safe_img_src(raw: &str) -> bool {
    use url::Url;
    let s = strip_url_controls(raw);
    let s = s.trim();
    if s.is_empty() {
        return true;
    }
    match Url::parse(s) {
        Ok(u) => match u.scheme() {
            "http" | "https" => true,
            "data" => {
                // u.path() returns the opaque path after "data:" — e.g.
                // "image/png;base64,…" for "data:image/png;base64,…".
                // SVG is intentionally absent: it can carry inline scripts.
                const SAFE_IMG_MIMES: &[&str] = &[
                    "image/png",
                    "image/jpeg",
                    "image/gif",
                    "image/webp",
                    "image/bmp",
                ];
                let path = u.path();
                SAFE_IMG_MIMES.iter().any(|mime| {
                    path.starts_with(mime) && path[mime.len()..].starts_with([';', ','])
                })
            }
            _ => false,
        },
        // Not an absolute URL — safe if there is no scheme-like prefix.
        Err(_) => !s.split('/').next().unwrap_or("").contains(':'),
    }
}

// ── SVG / PNG export ────────────────────────────────────────────────────────

/// All character-cell and rendering metrics used when building the SVG viewport.
///
/// Values are tuned for Courier New / DejaVu Sans Mono at 3× terminal scale.
/// Change a field here and the entire SVG renderer adjusts consistently.
struct SvgMetrics {
    /// Width of one monospace character cell in SVG user units (pixels).
    char_width: f64,
    /// Height of one line (cell) in SVG user units.
    line_height: f64,
    /// Font-size passed to the SVG `<style>` block.
    font_size: f64,
    /// Uniform padding (top / bottom / left / right) around the content area.
    pad: f64,
    /// Fraction of `line_height` from the top of a cell to the text baseline.
    baseline_ratio: f64,
    /// Thickness of box-drawing rule rects as a fraction of `line_height`.
    /// A ratio keeps rules proportional when `line_height` is changed.
    box_bar_ratio: f64,
}

const METRICS: SvgMetrics = SvgMetrics {
    char_width: 28.8,
    line_height: 60.0,
    font_size: 48.0,
    pad: 60.0,
    baseline_ratio: 0.78,
    box_bar_ratio: 0.07,
};

impl SvgMetrics {
    /// Half the width of one character cell — the x-distance from the left
    /// edge of a cell to its centre, and the width of each half-segment.
    const fn half_char_width(&self) -> f64 {
        self.char_width / 2.0
    }
    /// Half the height of one line cell — the y-distance from the top of a
    /// cell to its centre, and the height of each half-segment.
    const fn half_line_height(&self) -> f64 {
        self.line_height / 2.0
    }
}

/// Render a slice of `Line`s to a self-contained SVG string.
/// Each `StyledSpan` becomes a `<tspan>` with the correct colours and
/// font attributes; background colours get a `<rect>` drawn behind the text.
pub fn to_svg_string(lines: &[crate::style::Line], width: usize, theme: &Theme) -> String {
    use std::fmt::Write as FmtWrite;

    let m = &METRICS;
    let svg_w = width as f64 * m.char_width + m.pad * 2.0;
    let svg_h = lines.len() as f64 * m.line_height + m.pad * 2.0;

    // ── SVG envelope (write! for the structured header/footer) ──────────
    let mut svg = String::new();
    let _ = write!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">"#,
        w = svg_w as u32,
        h = svg_h as u32,
    );
    // Background fills the whole canvas so dark/light themes both look correct.
    let _ = write!(svg, r#"<rect width="{w}" height="{h}" fill="{bg}"/>"#,
        w = svg_w as u32, h = svg_h as u32, bg = color_css(theme.bg));
    // Embedded font definition — DejaVu Sans Mono is the fallback for systems
    // without Courier New.
    let _ = write!(
        svg,
        r#"<style>text{{font-family:'Courier New','DejaVu Sans Mono',Courier,monospace;font-size:{fs}px;dominant-baseline:auto;}}</style>"#,
        fs = m.font_size as u32,
    );

    // ── Line bodies — purely functional pipeline ─────────────────────────
    let body: String = lines
        .iter()
        .enumerate()
        .map(|(row, line)| {
            let y_top      = m.pad + row as f64 * m.line_height;
            let y_baseline = y_top + m.line_height * m.baseline_ratio;
            let bgs        = svg_bg_rects(line, y_top, m);
            let content    = if line.spans.is_empty() {
                String::new()
            } else if is_box_drawing_line(line) {
                svg_box_line(line, y_top, m, theme)
            } else {
                svg_text_line(line, y_baseline, m, theme)
            };
            bgs + &content
        })
        .collect();

    svg.push_str(&body);
    let _ = write!(svg, "</svg>");
    svg
}

/// Append a single `<rect>` element to an SVG string.
/// `x`/`y` are floored to integer pixels; `w`/`h` are ceiling-rounded so
/// adjacent rects share edges without leaving sub-pixel gaps.
fn svg_rect(x: f64, y: f64, w: f64, h: f64, fill: &str) -> String {
    format!(
        r#"<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{fill}"/>"#,
        x = x as u32,
        y = y as u32,
        w = w.ceil() as u32,
        h = h.ceil() as u32,
    )
}

/// Returns true if every non-space character in the line is a box-drawing glyph.
/// Used by `to_svg_string` to switch to rect-based rendering for table borders.
fn is_box_drawing_line(line: &crate::style::Line) -> bool {
    line.spans.iter().all(|s| {
        s.text.chars().all(|c| {
            matches!(
                c,
                ' ' | '─' | '│' | '╭' | '╮' | '╰' | '╯' | '├' | '┤' | '┬' | '┴' | '┼'
            )
        })
    }) && line.spans.iter().any(|s| s.text.chars().any(|c| c != ' '))
}

/// Which sides of a character cell a box-drawing glyph connects to.
struct BoxDirs {
    left: bool,
    right: bool,
    up: bool,
    down: bool,
}

/// Returns the connection directions for a box-drawing character.
fn box_char_dirs(c: char) -> BoxDirs {
    let (left, right, up, down) = match c {
        '─' => (true, true, false, false),
        '│' => (false, false, true, true),
        '╭' => (false, true, false, true), // arc: down + right
        '╮' => (true, false, false, true), // arc: down + left
        '╰' => (false, true, true, false), // arc: up   + right
        '╯' => (true, false, true, false), // arc: up   + left
        '├' => (false, true, true, true),
        '┤' => (true, false, true, true),
        '┬' => (true, true, false, true),
        '┴' => (true, true, true, false),
        '┼' => (true, true, true, true),
        _ => (false, false, false, false),
    };
    BoxDirs {
        left,
        right,
        up,
        down,
    }
}

/// Escape text content for safe embedding inside an SVG string.
fn svg_escape(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '&'  => "&amp;".to_string(),
            '<'  => "&lt;".to_string(),
            '>'  => "&gt;".to_string(),
            '"'  => "&quot;".to_string(),
            '\'' => "&#39;".to_string(),
            c    => c.to_string(),
        })
        .collect()
}

/// Build all background `<rect>` elements for spans that carry a bg colour.
/// Uses `scan` to track the running x-offset without a separate mutable variable.
fn svg_bg_rects(line: &crate::style::Line, y_top: f64, m: &SvgMetrics) -> String {
    line.spans
        .iter()
        .scan(0.0_f64, |col, span| {
            let span_w = UnicodeWidthStr::width(span.text.as_str()) as f64 * m.char_width;
            let x = m.pad + *col;
            *col += span_w;
            Some((x, span_w, span.style.bg))
        })
        .filter_map(|(x, span_w, bg)| {
            bg.map(|bg| svg_rect(x, y_top, span_w, m.line_height, &color_css(bg)))
        })
        .collect()
}

/// Build the SVG attribute string for one `<tspan>`.
fn svg_tspan_attrs(span: &crate::style::StyledSpan, x_pos: f64, theme: &Theme) -> String {
    let fg = color_css(span.style.fg.unwrap_or(theme.fg));
    let weight    = span.style.bold   .then_some(r#" font-weight="bold""#).unwrap_or("");
    let italic    = span.style.italic .then_some(r#" font-style="italic""#).unwrap_or("");
    let dim       = span.style.dim    .then_some(r#" opacity="0.5""#).unwrap_or("");
    let decoration = match (span.style.underline, span.style.strikethrough) {
        (true,  true)  => r#" text-decoration="underline line-through""#,
        (true,  false) => r#" text-decoration="underline""#,
        (false, true)  => r#" text-decoration="line-through""#,
        (false, false) => "",
    };
    format!(r#" x="{x}" fill="{fg}"{weight}{italic}{decoration}{dim}"#, x = x_pos as u32)
}

/// Render a line of styled text as `<text>…</text>` with one `<tspan>` per span.
/// `scan` carries the x-position forward through the span iterator.
fn svg_text_line(line: &crate::style::Line, y_baseline: f64, m: &SvgMetrics, theme: &Theme) -> String {
    let tspans: String = line
        .spans
        .iter()
        .scan(m.pad, |x_pos, span| {
            let span_w = UnicodeWidthStr::width(span.text.as_str()) as f64 * m.char_width;
            let attrs = svg_tspan_attrs(span, *x_pos, theme);
            *x_pos += span_w;
            Some(format!("<tspan{}>{}</tspan>", attrs, svg_escape(&span.text)))
        })
        .collect();
    format!(
        r#"<text y="{y}" xml:space="preserve">{tspans}</text>"#,
        y = y_baseline as u32,
    )
}

/// Render a box-drawing line as a series of pixel-perfect `<rect>` elements.
/// Uses `flat_map` + `scan` so there are no mutable variables outside iterators.
fn svg_box_line(line: &crate::style::Line, y_top: f64, m: &SvgMetrics, theme: &Theme) -> String {
    let bar_t  = (m.line_height * m.box_bar_ratio).max(2.0);
    let mid_y  = y_top + m.half_line_height();
    let half_bar = bar_t * 0.5;
    let fill = color_css(line.spans.iter().find_map(|s| s.style.fg).unwrap_or(theme.fg));

    line.spans
        .iter()
        .flat_map(|s| s.text.chars().collect::<Vec<_>>())
        .scan(m.pad, |cx, ch| {
            let dirs       = box_char_dirs(ch);
            let char_mid_x = *cx + m.half_char_width();
            let rects: Vec<String> = [
                dirs.left .then(|| svg_rect(*cx,                    mid_y - half_bar, m.half_char_width(), bar_t,             &fill)),
                dirs.right.then(|| svg_rect(*cx + m.half_char_width(), mid_y - half_bar, m.half_char_width(), bar_t,          &fill)),
                dirs.up   .then(|| svg_rect(char_mid_x - half_bar, y_top,             bar_t,             m.half_line_height(), &fill)),
                dirs.down .then(|| svg_rect(char_mid_x - half_bar, y_top + m.half_line_height(), bar_t, m.half_line_height(), &fill)),
            ]
            .into_iter()
            .flatten()
            .collect();
            *cx += m.char_width;
            Some(rects)
        })
        .flatten()
        .collect()
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

/// Physical dimensions and raster resolution for the ODP page layout.
///
/// All ODP slides use these values so they open correctly in LibreOffice,
/// Nextcloud, and PowerPoint regardless of the terminal width used to render.
struct PresLayout {
    /// Slide width in centimetres (ODF page layout).
    w_cm: f64,
    /// Slide height in centimetres (ODF page layout).
    h_cm: f64,
    /// PNG raster width in pixels used for `odp+png` export.
    png_w: u32,
    /// PNG raster height in pixels used for `odp+png` export.
    png_h: u32,
}

/// Standard 16:9 widescreen layout: 10 × 5.625 inches (25.4 × 14.288 cm),
/// rasterised at 4K (3840 × 2160) for sharp display on large monitors.
const LAYOUT: PresLayout = PresLayout {
    w_cm: 25.4,
    h_cm: 14.288,
    png_w: 3840,
    png_h: 2160,
};

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
            OdpImageKind::Png => svg_to_png_sized(&svg, theme.bg, LAYOUT.png_w, LAYOUT.png_h),
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
    zip.write_all(build_styles_xml(LAYOUT.w_cm, LAYOUT.h_cm).as_bytes())?;

    // content.xml — one <draw:page> per slide
    zip.start_file("content.xml", deflated)?;
    zip.write_all(build_content_xml(ext, ranges.len(), LAYOUT.w_cm, LAYOUT.h_cm).as_bytes())?;

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
