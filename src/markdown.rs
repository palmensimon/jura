use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Render a Jira wiki notation description into ratatui Lines.
///
/// `width` is the widget's inner width in columns; code blocks are padded to
/// this width so their background fills the full line (not just behind text).
///
/// Handles: headings (h1–h6), bold (*), italic (_), monospace ({{}}),
/// backtick inline code, unordered/ordered lists, horizontal rules,
/// {code:lang}…{code}, {noformat}…{noformat}, and {color} macros.
pub fn render(text: &str, width: u16) -> Vec<Line<'static>> {
    let width = width as usize;
    let text = normalize(text);

    let mut out: Vec<Line<'static>> = Vec::new();
    // (lang, accumulated lines)
    let mut code_block: Option<(String, Vec<String>)> = None;
    let mut ordered_counters: Vec<u32> = Vec::new(); // stack per depth

    for line in text.lines() {
        // ── Inside a code / noformat block ───────────────────────────────────
        if let Some((ref _lang, ref mut lines)) = code_block {
            let t = line.trim();
            if t.eq_ignore_ascii_case("{code}") || t.eq_ignore_ascii_case("{noformat}") {
                let (lang, lines) = code_block.take().unwrap();
                push_code_block(&mut out, &lang, lines, width);
            } else {
                lines.push(line.to_string());
            }
            continue;
        }

        // ── Block-level detection ─────────────────────────────────────────────

        // {code:lang} or bare {code}
        if let Some(lang) = try_code_open(line) {
            code_block = Some((lang, Vec::new()));
            ordered_counters.clear();
            continue;
        }

        // {noformat} opener (may also have params: {noformat:nopanel=true})
        if line.trim().to_ascii_lowercase().starts_with("{noformat") {
            code_block = Some((String::new(), Vec::new()));
            ordered_counters.clear();
            continue;
        }

        // h1. … h6.
        if let Some((level, content)) = try_heading(line) {
            ordered_counters.clear();
            push_heading(&mut out, level, content);
            continue;
        }

        // Unordered list: *, **, ***, or - (single level only)
        if let Some((depth, content)) = try_bullet(line) {
            // switching to bullet resets ordered counters
            ordered_counters.clear();
            let indent = "  ".repeat(depth.saturating_sub(1));
            let mut spans = vec![Span::styled(
                format!("{indent}• "),
                Style::default().fg(Color::DarkGray),
            )];
            spans.extend(parse_inline(content));
            out.push(Line::from(spans));
            continue;
        }

        // Ordered list: #, ##, ###
        if let Some((depth, content)) = try_ordered(line) {
            // Grow or shrink the counter stack
            while ordered_counters.len() < depth {
                ordered_counters.push(0);
            }
            ordered_counters.truncate(depth);
            *ordered_counters.last_mut().unwrap() += 1;
            let n = *ordered_counters.last().unwrap();
            let indent = "  ".repeat(depth.saturating_sub(1));
            let mut spans = vec![Span::styled(
                format!("{indent}{n}. "),
                Style::default().fg(Color::DarkGray),
            )];
            spans.extend(parse_inline(content));
            out.push(Line::from(spans));
            continue;
        }

        // ----  horizontal rule
        if line.trim() == "----" {
            ordered_counters.clear();
            out.push(Line::from(Span::styled(
                "─".repeat(60),
                Style::default().fg(Color::DarkGray),
            )));
            continue;
        }

        // Empty line
        if line.trim().is_empty() {
            ordered_counters.clear();
            out.push(Line::raw(""));
            continue;
        }

        // Regular text line — parse inline markup
        ordered_counters.clear();
        out.push(Line::from(parse_inline(line)));
    }

    // Flush any unclosed code block
    if let Some((lang, lines)) = code_block {
        push_code_block(&mut out, &lang, lines, width);
    }

    out
}

// ── Normalisation ─────────────────────────────────────────────────────────────

fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\u{00A0}', " ")
}

// ── Block helpers ─────────────────────────────────────────────────────────────

const CODE_BG: Color = Color::Rgb(30, 30, 30);

// U+2800 BRAILLE BLANK — visually empty but a real non-whitespace character.
// Terminal emulators and tmux cannot optimize it away as an "empty cell", so
// the background color is guaranteed to render even on otherwise-blank rows.
const BRAILLE_BLANK: char = '\u{2800}';

fn push_code_block(out: &mut Vec<Line<'static>>, lang: &str, lines: Vec<String>, width: usize) {
    // A fully blank row filled with Braille Blanks. fg == bg makes the
    // character invisible while forcing the dark background to render.
    let blank_row = || -> Line<'static> {
        Line::from(Span::styled(
            BRAILLE_BLANK.to_string().repeat(width),
            Style::default().fg(CODE_BG).bg(CODE_BG),
        ))
    };

    // Pad content to full width with regular spaces (bg is set alongside the
    // fg colour, so trailing spaces inherit the background correctly).
    let pad = |s: String| -> String {
        let n = s.chars().count();
        if width > n { format!("{}{}", s, " ".repeat(width - n)) } else { s }
    };

    // top padding
    out.push(blank_row());

    // language label
    let label = if lang.is_empty() { "code" } else { lang };
    out.push(Line::from(Span::styled(
        pad(format!("  {label}")),
        Style::default().fg(Color::DarkGray).bg(CODE_BG),
    )));

    // code content
    for line in &lines {
        if line.trim().is_empty() {
            // Blank line inside the block — must use Braille Blank too.
            out.push(blank_row());
        } else {
            out.push(Line::from(Span::styled(
                pad(format!("  {line}")),
                Style::default().fg(Color::Yellow).bg(CODE_BG),
            )));
        }
    }

    // bottom padding
    out.push(blank_row());

    // separator after the box
    out.push(Line::raw(""));
}

/// Returns the language string if `line` opens a code block.
fn try_code_open(line: &str) -> Option<String> {
    let t = line.trim();
    let lower = t.to_ascii_lowercase();
    if lower == "{code}" {
        return Some(String::new());
    }
    if lower.starts_with("{code:") {
        let rest = &t[6..]; // after "{code:"
        let end = rest.find('}').unwrap_or(rest.len());
        let params = &rest[..end];
        // First param before '|'; strip optional "language=" prefix
        let lang_raw = params.split('|').next().unwrap_or("").trim();
        let lang = lang_raw
            .strip_prefix("language=")
            .unwrap_or(lang_raw)
            .to_string();
        return Some(lang);
    }
    None
}

/// Returns `(level, text_after_marker)` for Jira headings `h1. ` … `h6. `.
fn try_heading(line: &str) -> Option<(u8, &str)> {
    if line.len() < 4 {
        return None;
    }
    let b = line.as_bytes();
    if b[0] != b'h' {
        return None;
    }
    let level = b[1].wrapping_sub(b'0');
    if level == 0 || level > 6 {
        return None;
    }
    if b.get(2) != Some(&b'.') || b.get(3) != Some(&b' ') {
        return None;
    }
    Some((level, line[4..].trim()))
}

fn push_heading(out: &mut Vec<Line<'static>>, level: u8, content: &str) {
    let color = if level <= 2 { Color::Cyan } else { Color::Blue };
    let prefix = format!("{} ", "#".repeat(level as usize));
    let mut spans = vec![Span::styled(prefix, Style::default().fg(Color::DarkGray))];
    for s in parse_inline(content) {
        spans.push(Span::styled(
            s.content.into_owned(),
            s.style.fg(color).add_modifier(Modifier::BOLD),
        ));
    }
    out.push(Line::from(spans));
    out.push(Line::raw(""));
}

/// Returns `(depth, item_text)` for `* item`, `** item`, `- item`.
fn try_bullet(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_start();

    // Star-based: *, **, ***  (must be followed by a space)
    let stars = trimmed.chars().take_while(|&c| c == '*').count();
    if stars > 0 {
        let after = &trimmed[stars..];
        if after.starts_with(' ') {
            return Some((stars, after[1..].trim_start()));
        }
    }

    // Dash: "- item" (single level)
    if let Some(rest) = trimmed.strip_prefix("- ") {
        return Some((1, rest));
    }

    None
}

/// Returns `(depth, item_text)` for `# item`, `## item`.
fn try_ordered(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_start();
    let hashes = trimmed.chars().take_while(|&c| c == '#').count();
    if hashes == 0 {
        return None;
    }
    let after = &trimmed[hashes..];
    if after.starts_with(' ') {
        Some((hashes, after[1..].trim_start()))
    } else {
        None
    }
}

// ── Inline parser ─────────────────────────────────────────────────────────────
//
// Jira wiki notation inline markup (in priority order):
//   {{mono}}      monospace / inline code → Yellow
//   `backtick`    common in this Jira instance → Yellow
//   *bold*        bold
//   _italic_      italic (guarded against mid-identifier _)
//   [label|url]   link → Blue underlined (shows label or raw url)
//   {color:…}…{color}   ignored (colour macro, stripped)
//   {panel:…}…{panel}   ignored (panel macro, stripped)
//   Everything else: literal

pub fn parse_inline(text: &str) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut plain = String::new();
    let mut remaining: &str = text;

    macro_rules! flush {
        () => {
            if !plain.is_empty() {
                append_plain(&mut spans, std::mem::take(&mut plain));
            }
        };
    }

    while !remaining.is_empty() {
        // {{monospace}}
        if remaining.starts_with("{{") {
            if let Some(close) = remaining[2..].find("}}") {
                flush!();
                let content = remaining[2..close + 2].to_string();
                spans.push(Span::styled(content, Style::default().fg(Color::Yellow)));
                remaining = &remaining[close + 4..];
                continue;
            }
        }

        // `backtick code`
        if remaining.starts_with('`') {
            if let Some(close) = remaining[1..].find('`') {
                flush!();
                let content = remaining[1..close + 1].to_string();
                spans.push(Span::styled(content, Style::default().fg(Color::Yellow)));
                remaining = &remaining[close + 2..];
                continue;
            }
        }

        // *bold*  — opening * must not be followed by space
        if remaining.starts_with('*') {
            let inner = &remaining[1..];
            if !inner.starts_with(' ') && !inner.is_empty() {
                if let Some(close) = find_close(inner, '*') {
                    flush!();
                    let content = inner[..close].to_string();
                    spans.push(Span::styled(
                        content,
                        Style::default().add_modifier(Modifier::BOLD),
                    ));
                    remaining = &inner[close + 1..];
                    continue;
                }
            }
        }

        // _italic_  — guard against mid-word underscores (url_like_this)
        if remaining.starts_with('_') {
            let inner = &remaining[1..];
            let prev_is_alnum = plain.chars().last().map(|c| c.is_alphanumeric()).unwrap_or(false);
            if !prev_is_alnum && !inner.starts_with(' ') && !inner.is_empty() {
                if let Some(close) = find_close(inner, '_') {
                    flush!();
                    let content = inner[..close].to_string();
                    spans.push(Span::styled(
                        content,
                        Style::default().add_modifier(Modifier::ITALIC),
                    ));
                    remaining = &inner[close + 1..];
                    continue;
                }
            }
        }

        // [label|url] or [url]
        if remaining.starts_with('[') {
            if let Some(close) = remaining.find(']') {
                let inner = &remaining[1..close];
                flush!();
                let display = inner
                    .split_once('|')
                    .map(|(label, _)| label)
                    .unwrap_or(inner)
                    .to_string();
                spans.push(Span::styled(
                    display,
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::UNDERLINED),
                ));
                remaining = &remaining[close + 1..];
                continue;
            }
        }

        // {color:…} opening tag — skip the tag, keep the text until {color}
        if remaining.starts_with("{color") {
            if let Some(close_brace) = remaining.find('}') {
                remaining = &remaining[close_brace + 1..];
                continue;
            }
        }

        // {panel…} / other macros — skip the tag only
        if remaining.starts_with('{') {
            if let Some(close_brace) = remaining.find('}') {
                // Only skip short macro-like tags (not prose that starts with {)
                if close_brace < 40 {
                    remaining = &remaining[close_brace + 1..];
                    continue;
                }
            }
        }

        // Default: consume one Unicode scalar
        let width = remaining.chars().next().map(|c| c.len_utf8()).unwrap_or(1);
        plain.push_str(&remaining[..width]);
        remaining = &remaining[width..];
    }

    flush!();
    spans
}

/// Append text to the last plain span if possible, otherwise create a new one.
fn append_plain(spans: &mut Vec<Span<'static>>, text: String) {
    if let Some(last) = spans.last_mut() {
        if last.style == Style::default() {
            let combined = last.content.to_string() + &text;
            *last = Span::raw(combined);
            return;
        }
    }
    spans.push(Span::raw(text));
}

/// Find the byte offset of `delim` in `text` such that the character
/// immediately before it is not a space (Jira requires no space before
/// the closing delimiter).
fn find_close(text: &str, delim: char) -> Option<usize> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    for (i, &(byte_pos, ch)) in chars.iter().enumerate() {
        if ch == delim && i > 0 && chars[i - 1].1 != ' ' {
            return Some(byte_pos);
        }
    }
    None
}
