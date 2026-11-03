//! PDF writer for the active editor document (PDF 1.4, A4, embedded TrueType, Unicode).
//!
//! Two input forms are accepted:
//!
//! * **NXPDF markup**: the text starts with the line `%%nxpdf 1`. It is laid out in a
//!   proportional font with headings, paragraphs, bold/italic, colours, explicit page
//!   breaks and page headers/footers with page numbers (see `docs/ide/plugins.md`).
//! * **Plain text**: anything else. It is rendered verbatim in a monospace font with a
//!   centred `n / N` footer; no character is interpreted.
//!
//! The font is embedded as a CID TrueType font (Identity-H) with a `ToUnicode` map, so
//! text stays selectable and every character the font covers (Latin, Greek, Cyrillic,
//! ...) is rendered; characters without a glyph show the font's "missing" box. Bold is
//! synthesised by stroking the glyphs and italic by slanting them. Output is
//! deterministic. Input size and page count are bounded; exceeding either is an error,
//! never a truncation.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;
use std::sync::OnceLock;

use epaint_default_fonts::{HACK_REGULAR, UBUNTU_LIGHT};
use flate2::Compression;
use flate2::write::ZlibEncoder;
use ttf_parser::{Face, GlyphId};

use super::project_io::write_project_file;

pub const EXTENSION: &str = "pdf";
pub const MAX_TEXT_BYTES: usize = 1024 * 1024;
pub const MAX_PAGES: usize = 400;
/// First line of a document that is interpreted as NXPDF markup.
pub const MARKUP_MAGIC: &str = "%%nxpdf 1";
const MAX_HEADER_CHARS: usize = 200;

const PAGE_WIDTH: f32 = 595.0;
const PAGE_HEIGHT: f32 = 842.0;
const MARGIN: f32 = 50.0;
const CONTENT_WIDTH: f32 = PAGE_WIDTH - 2.0 * MARGIN;
const HEADER_BASELINE: f32 = PAGE_HEIGHT - 32.0;
const FOOTER_BASELINE: f32 = 30.0;
const HEADER_SIZE: f32 = 9.0;
const BODY_SIZE: f32 = 11.0;
const BODY_LEADING: f32 = 15.4;
const HEADING_SIZES: [f32; 3] = [20.0, 16.0, 13.0];
const PLAIN_SIZE: f32 = 9.0;
const PLAIN_LEADING: f32 = 11.0;
const TAB_WIDTH: usize = 4;
const HEADER_COLOR: [u8; 3] = [0x55, 0x55, 0x55];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfSummary {
    pub path: String,
    pub pages: usize,
    pub bytes: u64,
}

struct Font {
    data: &'static [u8],
    face: Face<'static>,
    name: &'static str,
    units: f32,
}

impl Font {
    fn new(data: &'static [u8], name: &'static str) -> Self {
        let face = Face::parse(data, 0).expect("bundled font is valid");
        let units = f32::from(face.units_per_em());
        Self {
            data,
            face,
            name,
            units,
        }
    }

    fn glyph(&self, ch: char) -> u16 {
        self.face.glyph_index(ch).map_or(0, |glyph| glyph.0)
    }

    /// Advance in 1/1000 em.
    fn advance(&self, glyph: u16) -> f32 {
        f32::from(self.face.glyph_hor_advance(GlyphId(glyph)).unwrap_or(0)) * 1000.0 / self.units
    }

    fn width(&self, ch: char, size: f32) -> f32 {
        self.advance(self.glyph(ch)) * size / 1000.0
    }
}

#[derive(Clone, Copy, PartialEq)]
struct Cell {
    ch: char,
    bold: bool,
    italic: bool,
    color: Option<[u8; 3]>,
}

struct Line {
    cells: Vec<Cell>,
    size: f32,
    leading: f32,
    before: f32,
}

enum Item {
    Line(Line),
    PageBreak,
}

#[derive(Default)]
struct Document {
    items: Vec<Item>,
    header: Option<String>,
    footer: Option<String>,
}

fn parse_color(spec: &str) -> Option<[u8; 3]> {
    let digits = spec.strip_prefix('#')?;
    if !digits.is_ascii() || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let value = |text: &str| u8::from_str_radix(text, 16).ok();
    match digits.len() {
        3 => {
            let channel = |index: usize| value(&digits[index..=index]).map(|v| v * 17);
            Some([channel(0)?, channel(1)?, channel(2)?])
        }
        6 => Some([
            value(&digits[0..2])?,
            value(&digits[2..4])?,
            value(&digits[4..6])?,
        ]),
        _ => None,
    }
}

/// Parses inline markup: `**bold**`, `*italic*`, `{#rrggbb}...{/}`, `\n` (line break)
/// and backslash escapes for `\ * { } | # @`. Styles still open at the end are closed.
fn parse_inline(source: &str, bold_base: bool) -> Vec<Cell> {
    let chars: Vec<char> = source.chars().collect();
    let mut cells = Vec::with_capacity(chars.len());
    let (mut bold, mut italic) = (false, false);
    let mut colors: Vec<[u8; 3]> = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        let ch = chars[index];
        let mut literal = Some(ch);
        match ch {
            '\\' => match chars.get(index + 1) {
                Some(&next) if "\\*{}|#@".contains(next) => {
                    literal = Some(next);
                    index += 1;
                }
                Some('n') => {
                    literal = Some('\n');
                    index += 1;
                }
                _ => {}
            },
            '*' => {
                literal = None;
                if chars.get(index + 1) == Some(&'*') {
                    bold = !bold;
                    index += 1;
                } else {
                    italic = !italic;
                }
            }
            '{' => {
                if chars.get(index + 1..index + 3) == Some(&['/', '}']) {
                    colors.pop();
                    literal = None;
                    index += 2;
                } else if let Some(end) = chars[index..].iter().position(|&c| c == '}') {
                    let spec: String = chars[index + 1..index + end].iter().collect();
                    if let Some(color) = parse_color(&spec) {
                        colors.push(color);
                        literal = None;
                        index += end;
                    }
                }
            }
            _ => {}
        }
        if let Some(ch) = literal {
            let color = colors.last().copied();
            match ch {
                '\t' => {
                    for _ in 0..TAB_WIDTH {
                        cells.push(cell(' ', bold ^ bold_base, italic, color));
                    }
                }
                '\n' => cells.push(cell('\n', false, false, None)),
                c if c.is_control() => {}
                c => cells.push(cell(c, bold ^ bold_base, italic, color)),
            }
        }
        index += 1;
    }
    cells
}

fn cell(ch: char, bold: bool, italic: bool, color: Option<[u8; 3]>) -> Cell {
    Cell {
        ch,
        bold,
        italic,
        color,
    }
}

fn cells_width(font: &Font, cells: &[Cell], size: f32) -> f32 {
    cells.iter().map(|c| font.width(c.ch, size)).sum()
}

/// Breaks `cells` into lines no wider than `max`. With `word_wrap` lines break at
/// spaces where possible; a `\n` cell always ends the line.
fn wrap(font: &Font, cells: &[Cell], size: f32, max: f32, word_wrap: bool) -> Vec<Vec<Cell>> {
    let mut lines = Vec::new();
    let mut current: Vec<Cell> = Vec::new();
    let mut width = 0.0;
    let mut last_space: Option<usize> = None;
    for &item in cells {
        if item.ch == '\n' {
            lines.push(std::mem::take(&mut current));
            width = 0.0;
            last_space = None;
            continue;
        }
        let advance = font.width(item.ch, size);
        if width + advance > max && !current.is_empty() {
            if word_wrap && item.ch == ' ' {
                lines.push(std::mem::take(&mut current));
                width = 0.0;
                last_space = None;
                continue;
            }
            match last_space.filter(|_| word_wrap) {
                Some(at) => {
                    let tail = current.split_off(at + 1);
                    while current.last().is_some_and(|c| c.ch == ' ') {
                        current.pop();
                    }
                    lines.push(std::mem::replace(&mut current, tail));
                    width = cells_width(font, &current, size);
                }
                None => {
                    lines.push(std::mem::take(&mut current));
                    width = 0.0;
                }
            }
            last_space = None;
        }
        current.push(item);
        width += advance;
        if item.ch == ' ' {
            last_space = Some(current.len() - 1);
        }
    }
    lines.push(current);
    lines
}

fn plain_document(font: &Font, text: &str) -> Document {
    let mut document = Document {
        footer: Some("{page} / {pages}".to_owned()),
        ..Document::default()
    };
    for raw in text.split('\n') {
        let mut cells: Vec<Cell> = Vec::new();
        for ch in raw.chars() {
            match ch {
                '\t' => {
                    for _ in 0..TAB_WIDTH - cells.len() % TAB_WIDTH {
                        cells.push(cell(' ', false, false, None));
                    }
                }
                c if c.is_control() => {}
                c => cells.push(cell(c, false, false, None)),
            }
        }
        for line in wrap(font, &cells, PLAIN_SIZE, CONTENT_WIDTH, false) {
            document.items.push(Item::Line(Line {
                cells: line,
                size: PLAIN_SIZE,
                leading: PLAIN_LEADING,
                before: 0.0,
            }));
        }
    }
    document
}
fn flush_paragraph(font: &Font, document: &mut Document, paragraph: &mut Vec<&str>) {
    if paragraph.is_empty() {
        return;
    }
    let joined = paragraph.join(" ");
    paragraph.clear();
    push_block(
        font,
        document,
        &parse_inline(&joined, false),
        BODY_SIZE,
        BODY_LEADING,
        6.0,
    );
}

fn markup_document(font: &Font, text: &str) -> Result<Document, &'static str> {
    let mut document = Document::default();
    let mut paragraph: Vec<&str> = Vec::new();
    let mut lines = text.split('\n').map(|line| line.trim_end_matches('\r'));
    lines.next();

    for line in lines {
        let trimmed = line.trim();
        if trimmed.starts_with("%%") {
            continue;
        }
        if trimmed.is_empty() {
            flush_paragraph(font, &mut document, &mut paragraph);
        } else if trimmed == "@pagebreak" {
            flush_paragraph(font, &mut document, &mut paragraph);
            document.items.push(Item::PageBreak);
        } else if let Some(rest) = directive(trimmed, "@header") {
            flush_paragraph(font, &mut document, &mut paragraph);
            document.header = Some(rest.to_owned());
        } else if let Some(rest) = directive(trimmed, "@footer") {
            flush_paragraph(font, &mut document, &mut paragraph);
            document.footer = Some(rest.to_owned());
        } else if let Some((level, rest)) = heading(trimmed) {
            flush_paragraph(font, &mut document, &mut paragraph);
            let size = HEADING_SIZES[level - 1];
            push_block(
                font,
                &mut document,
                &parse_inline(rest, true),
                size,
                size * 1.3,
                size * 0.8,
            );
        } else {
            paragraph.push(trimmed);
        }
    }
    flush_paragraph(font, &mut document, &mut paragraph);
    for template in [&document.header, &document.footer].into_iter().flatten() {
        if template.chars().count() > MAX_HEADER_CHARS {
            return Err("PDF header or footer is too long");
        }
        header_zones(font, template, "0", "0")?;
    }
    Ok(document)
}

fn directive<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(name)?;
    if rest.is_empty() {
        Some("")
    } else {
        rest.strip_prefix(' ').map(str::trim)
    }
}

fn heading(line: &str) -> Option<(usize, &str)> {
    let level = line.chars().take_while(|&c| c == '#').count();
    let rest = line[level..].strip_prefix(' ')?;
    (1..=3).contains(&level).then_some((level, rest.trim()))
}

fn push_block(
    font: &Font,
    document: &mut Document,
    cells: &[Cell],
    size: f32,
    leading: f32,
    before: f32,
) {
    for (index, line) in wrap(font, cells, size, CONTENT_WIDTH, true)
        .into_iter()
        .enumerate()
    {
        document.items.push(Item::Line(Line {
            cells: line,
            size,
            leading,
            before: if index == 0 { before } else { 0.0 },
        }));
    }
}

/// Alignment zones of a header/footer: `left | centre | right` split at unescaped `|`.
/// One part is centred, two parts are left and right.
fn header_zones(
    font: &Font,
    template: &str,
    page: &str,
    pages: &str,
) -> Result<Vec<(f32, Vec<Cell>)>, &'static str> {
    let text = template.replace("{page}", page).replace("{pages}", pages);
    let mut parts: Vec<String> = vec![String::new()];
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => {
                parts.last_mut().unwrap().push(ch);
                if let Some(next) = chars.next() {
                    parts.last_mut().unwrap().push(next);
                }
            }
            '|' => parts.push(String::new()),
            _ => parts.last_mut().unwrap().push(ch),
        }
    }
    let alignment: &[f32] = match parts.len() {
        1 => &[0.5],
        2 => &[0.0, 1.0],
        3 => &[0.0, 0.5, 1.0],
        _ => return Err("PDF header or footer has more than three zones"),
    };
    Ok(parts
        .iter()
        .zip(alignment)
        .filter(|(part, _)| !part.trim().is_empty())
        .map(|(part, &align)| {
            let mut cells = parse_inline(part.trim(), false);
            cells.retain(|c| c.ch != '\n');
            let width = cells_width(font, &cells, HEADER_SIZE);
            (MARGIN + (CONTENT_WIDTH - width) * align, cells)
        })
        .collect())
}

type Page<'a> = Vec<(f32, &'a Line)>;

fn paginate(items: &[Item]) -> Result<Vec<Page<'_>>, &'static str> {
    const TOP: f32 = PAGE_HEIGHT - MARGIN;
    let mut pages: Vec<Page<'_>> = vec![Vec::new()];
    let mut cursor = TOP;
    let mut pending_break = false;
    for item in items {
        match item {
            Item::PageBreak => pending_break = true,
            Item::Line(line) => {
                let mut before = line.before;
                if pages.last().unwrap().is_empty() {
                    before = 0.0;
                } else if pending_break || cursor - before - line.leading < MARGIN {
                    pages.push(Vec::new());
                    cursor = TOP;
                    before = 0.0;
                }
                pending_break = false;
                if pages.len() > MAX_PAGES {
                    return Err("document exceeds the PDF page limit");
                }
                let top = cursor - before;
                pages.last_mut().unwrap().push((top - line.size, line));
                cursor = top - line.leading;
            }
        }
    }
    Ok(pages)
}

fn color_ops(color: [u8; 3]) -> String {
    let [r, g, b] = color.map(|channel| f32::from(channel) / 255.0);
    format!("{r:.3} {g:.3} {b:.3}")
}

struct Emitter<'a> {
    font: &'a Font,
    used: BTreeMap<u16, char>,
}

impl Emitter<'_> {
    fn text(
        &mut self,
        out: &mut String,
        cells: &[Cell],
        x: f32,
        y: f32,
        size: f32,
        default_color: [u8; 3],
    ) {
        let mut x = x;
        let mut start = 0;
        while start < cells.len() {
            let first = cells[start];
            let style = (first.bold, first.italic, first.color);
            let end = cells[start..]
                .iter()
                .position(|c| (c.bold, c.italic, c.color) != style)
                .map_or(cells.len(), |offset| start + offset);
            let run = &cells[start..end];
            let color = color_ops(first.color.unwrap_or(default_color));
            let slant = if first.italic { 0.2 } else { 0.0 };
            let _ = write!(out, "BT\n/F1 {size:.2} Tf\n{color} rg\n");
            if first.bold {
                let _ = write!(out, "{color} RG\n{:.2} w\n2 Tr\n", size * 0.03);
            }
            let _ = write!(out, "1 0 {slant:.2} 1 {x:.2} {y:.2} Tm\n<");
            for item in run {
                let glyph = self.font.glyph(item.ch);
                if glyph != 0 {
                    self.used.entry(glyph).or_insert(item.ch);
                }
                let _ = write!(out, "{glyph:04X}");
            }
            out.push_str("> Tj\nET\n");
            x += cells_width(self.font, run, size);
            start = end;
        }
    }
}

fn to_unicode_cmap(used: &BTreeMap<u16, char>) -> String {
    let mut cmap = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n\
         /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
         /CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n\
         1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    let entries: Vec<_> = used.iter().collect();
    for chunk in entries.chunks(100) {
        let _ = writeln!(cmap, "{} beginbfchar", chunk.len());
        for (glyph, ch) in chunk {
            let mut units = [0u16; 2];
            let hex: String = ch
                .encode_utf16(&mut units)
                .iter()
                .map(|unit| format!("{unit:04X}"))
                .collect();
            let _ = writeln!(cmap, "<{glyph:04X}> <{hex}>");
        }
        cmap.push_str("endbfchar\n");
    }
    cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    cmap
}

fn compressed_font(font: &Font) -> Vec<u8> {
    static CACHE: OnceLock<Vec<(&'static str, Vec<u8>)>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| {
        [
            (UBUNTU_LIGHT, "UbuntuLight"),
            (HACK_REGULAR, "Hack-Regular"),
        ]
        .into_iter()
        .map(|(data, name)| {
            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::new(6));
            encoder.write_all(data).expect("in-memory write");
            (name, encoder.finish().expect("in-memory compression"))
        })
        .collect()
    });
    cache
        .iter()
        .find(|(name, _)| *name == font.name)
        .map(|(_, bytes)| bytes.clone())
        .expect("font is cached")
}

fn stream_object(dictionary: &str, data: &[u8]) -> Vec<u8> {
    let mut object = format!("<< {dictionary} /Length {} >>\nstream\n", data.len()).into_bytes();
    object.extend_from_slice(data);
    object.extend_from_slice(b"\nendstream");
    object
}
/// Renders `text` as a PDF file image and returns it with its page count.
pub fn render(text: &str) -> Result<(Vec<u8>, usize), &'static str> {
    if text.len() > MAX_TEXT_BYTES {
        return Err("document exceeds the PDF size limit");
    }
    let body = text.strip_prefix('\u{feff}').unwrap_or(text);
    let first = body.split('\n').next().unwrap_or("").trim();
    let markup = first.starts_with("%%nxpdf");
    if markup && first != MARKUP_MAGIC {
        return Err("unsupported NXPDF markup version");
    }
    let font = if markup {
        Font::new(UBUNTU_LIGHT, "UbuntuLight")
    } else {
        Font::new(HACK_REGULAR, "Hack-Regular")
    };
    let document = if markup {
        markup_document(&font, body)?
    } else {
        plain_document(&font, body)
    };
    let pages = paginate(&document.items)?;
    let total = pages.len();

    let mut emitter = Emitter {
        font: &font,
        used: BTreeMap::new(),
    };
    let mut streams = Vec::with_capacity(total);
    for (index, page) in pages.iter().enumerate() {
        let mut stream = String::new();
        let number = (index + 1).to_string();
        let count = total.to_string();
        for (template, baseline) in [
            (&document.header, HEADER_BASELINE),
            (&document.footer, FOOTER_BASELINE),
        ] {
            if let Some(template) = template {
                for (x, cells) in header_zones(&font, template, &number, &count)? {
                    emitter.text(&mut stream, &cells, x, baseline, HEADER_SIZE, HEADER_COLOR);
                }
            }
        }
        for (baseline, line) in page {
            emitter.text(
                &mut stream,
                &line.cells,
                MARGIN,
                *baseline,
                line.size,
                [0, 0, 0],
            );
        }
        streams.push(stream);
    }

    let face = &font.face;
    let bbox = face.global_bounding_box();
    let scale = 1000.0 / font.units;
    let scaled = |value: i16| (f32::from(value) * scale).round() as i32;
    let widths: String = emitter
        .used
        .keys()
        .map(|&glyph| format!("{glyph} [{}]", font.advance(glyph).round() as i32))
        .collect::<Vec<_>>()
        .join(" ");
    let ascent = scaled(face.ascender());
    let cap_height = face.capital_height().map_or(ascent, scaled);

    // Objects: 1 catalog, 2 page tree, 3-7 font, then (page, content) pairs.
    let mut bodies: Vec<Vec<u8>> = Vec::with_capacity(7 + total * 2);
    let kids: Vec<String> = (0..total).map(|n| format!("{} 0 R", 8 + n * 2)).collect();
    bodies.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
    bodies.push(
        format!(
            "<< /Type /Pages /Kids [{}] /Count {total} >>",
            kids.join(" ")
        )
        .into_bytes(),
    );
    bodies.push(
        format!(
            "<< /Type /Font /Subtype /Type0 /BaseFont /{name} /Encoding /Identity-H \
             /DescendantFonts [4 0 R] /ToUnicode 7 0 R >>",
            name = font.name
        )
        .into_bytes(),
    );
    bodies.push(
        format!(
            "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /{name} \
             /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> \
             /FontDescriptor 5 0 R /CIDToGIDMap /Identity /DW 500 /W [{widths}] >>",
            name = font.name
        )
        .into_bytes(),
    );
    bodies.push(
        format!(
            "<< /Type /FontDescriptor /FontName /{name} /Flags 32 \
             /FontBBox [{} {} {} {}] /ItalicAngle 0 /Ascent {ascent} /Descent {} \
             /CapHeight {cap_height} /StemV 80 /FontFile2 6 0 R >>",
            scaled(bbox.x_min),
            scaled(bbox.y_min),
            scaled(bbox.x_max),
            scaled(bbox.y_max),
            scaled(face.descender()),
            name = font.name
        )
        .into_bytes(),
    );
    bodies.push(stream_object(
        &format!("/Length1 {} /Filter /FlateDecode", font.data.len()),
        &compressed_font(&font),
    ));
    bodies.push(stream_object("", to_unicode_cmap(&emitter.used).as_bytes()));
    for (n, stream) in streams.iter().enumerate() {
        bodies.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {PAGE_WIDTH} {PAGE_HEIGHT}] \
                 /Resources << /Font << /F1 3 0 R >> >> /Contents {} 0 R >>",
                9 + n * 2
            )
            .into_bytes(),
        );
        bodies.push(stream_object("", stream.as_bytes()));
    }

    let mut out = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::with_capacity(bodies.len());
    for (index, body) in bodies.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", index + 1).as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    let mut table = format!("xref\n0 {}\n0000000000 65535 f \n", bodies.len() + 1);
    for offset in offsets {
        let _ = writeln!(table, "{offset:010} 00000 n ");
    }
    let _ = write!(
        table,
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        bodies.len() + 1
    );
    out.extend_from_slice(table.as_bytes());
    Ok((out, total))
}

/// Renders `text` and writes it to the project-relative `.pdf` path `output`.
pub fn export(root: &Path, output: &str, text: &str) -> Result<PdfSummary, &'static str> {
    let (bytes, pages) = render(text)?;
    write_project_file(root, output, EXTENSION, &bytes)?;
    Ok(PdfSummary {
        path: output.to_owned(),
        pages,
        bytes: bytes.len() as u64,
    })
}
#[cfg(test)]
mod tests {
    use std::fs;

    use super::super::project_io::test_support::TempProject;
    use super::*;

    fn find(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
        haystack[from..]
            .windows(needle.len())
            .position(|window| window == needle)
            .map(|position| position + from)
    }

    /// Checks header, trailer, and that every xref offset points at `N 0 obj`.
    fn assert_structurally_valid(pdf: &[u8]) -> usize {
        assert!(pdf.starts_with(b"%PDF-1.4\n"));
        assert!(pdf.ends_with(b"%%EOF\n"));
        let start = find(pdf, b"startxref\n", 0).unwrap() + 10;
        let end = find(pdf, b"\n", start).unwrap();
        let xref: usize = std::str::from_utf8(&pdf[start..end])
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(&pdf[xref..xref + 5], b"xref\n");
        let text = String::from_utf8_lossy(&pdf[xref..]).into_owned();
        let mut lines = text.lines().skip(1);
        let count: usize = lines
            .next()
            .unwrap()
            .split(' ')
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(lines.next().unwrap(), "0000000000 65535 f ");
        for number in 1..count {
            let entry = lines.next().unwrap();
            assert_eq!(entry.len(), 19, "entries are 20 bytes with the newline");
            let offset: usize = entry[..10].parse().unwrap();
            let expected = format!("{number} 0 obj\n");
            assert_eq!(&pdf[offset..offset + expected.len()], expected.as_bytes());
        }
        assert!(text.contains(&format!("/Size {count} /Root 1 0 R")));
        count
    }

    /// Verifies every stream's `/Length` and returns the decoded (inflated) payloads.
    fn streams(pdf: &[u8]) -> Vec<Vec<u8>> {
        use std::io::Read as _;
        let mut found = Vec::new();
        let mut at = 0;
        while let Some(object) = find(pdf, b" 0 obj\n", at) {
            let end = find(pdf, b"\nendobj\n", object).unwrap();
            let body = &pdf[object + 7..end];
            if let Some(stream) = find(body, b"\nstream\n", 0) {
                let dictionary = String::from_utf8_lossy(&body[..stream]).into_owned();
                let length: usize = dictionary
                    .split("/Length ")
                    .nth(1)
                    .and_then(|rest| rest.split(' ').next())
                    .unwrap()
                    .parse()
                    .unwrap();
                let data = &body[stream + 8..];
                assert_eq!(&data[length..], b"\nendstream");
                let payload = &data[..length];
                if dictionary.contains("/FlateDecode") {
                    let mut inflated = Vec::new();
                    flate2::read::ZlibDecoder::new(payload)
                        .read_to_end(&mut inflated)
                        .unwrap();
                    found.push(inflated);
                } else {
                    found.push(payload.to_vec());
                }
            }
            at = end + 8;
        }
        found
    }

    /// Streams are: font file, ToUnicode, then one content stream per page.
    fn page_streams(pdf: &[u8]) -> Vec<String> {
        streams(pdf)
            .into_iter()
            .skip(2)
            .map(|data| String::from_utf8(data).unwrap())
            .collect()
    }

    fn hex(font: &Font, text: &str) -> String {
        text.chars()
            .map(|ch| format!("{:04X}", font.glyph(ch)))
            .collect()
    }

    fn sans() -> Font {
        Font::new(UBUNTU_LIGHT, "UbuntuLight")
    }

    fn mono() -> Font {
        Font::new(HACK_REGULAR, "Hack-Regular")
    }

    #[test]
    fn plain_text_fallback_is_valid_monospace_with_footer() {
        let (pdf, pages) = render("hello (world) \\ back").unwrap();
        assert_eq!(pages, 1);
        assert_eq!(assert_structurally_valid(&pdf), 7 + 2 + 1);
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains("/BaseFont /Hack-Regular"));
        assert!(text.contains("/Count 1"));
        let content = &page_streams(&pdf)[0];
        assert!(content.contains(&hex(&mono(), "hello (world) \\ back")));
        assert!(content.contains(&hex(&mono(), "1 / 1")));
    }

    #[test]
    fn plain_text_is_never_interpreted_as_markup() {
        let (pdf, _) = render("# not a heading **x**\n@pagebreak").unwrap();
        let content = &page_streams(&pdf)[0];
        assert!(content.contains(&hex(&mono(), "# not a heading **x**")));
        assert!(content.contains(&hex(&mono(), "@pagebreak")));
        assert!(!content.contains("2 Tr"));
    }

    #[test]
    fn long_documents_paginate_and_streams_are_consistent() {
        let per_page = ((PAGE_HEIGHT - 2.0 * MARGIN) / PLAIN_LEADING) as usize;
        let text = (0..per_page * 2 + 1)
            .map(|n| format!("line {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        let (pdf, pages) = render(&text).unwrap();
        assert_eq!(pages, 3);
        assert_eq!(assert_structurally_valid(&pdf), 1 + 1 + 5 + 3 * 2 + 1);
        assert_eq!(page_streams(&pdf).len(), 3);
        assert!(page_streams(&pdf)[2].contains(&hex(&mono(), "3 / 3")));
    }

    #[test]
    fn long_plain_lines_wrap_and_tabs_expand() {
        let font = mono();
        let doc = plain_document(&font, &"x".repeat(200));
        assert!(doc.items.len() >= 3);
        let doc = plain_document(&font, "a\tb");
        let Item::Line(line) = &doc.items[0] else {
            panic!()
        };
        assert_eq!(line.cells.iter().map(|c| c.ch).collect::<String>(), "a   b");
    }

    #[test]
    fn unicode_is_embedded_with_a_tounicode_map() {
        let font = sans();
        for ch in ['\u{e4}', '\u{df}', '\u{416}', '\u{3c9}', '\u{e9}'] {
            assert_ne!(font.glyph(ch), 0, "{ch} has a glyph");
        }
        let sample = "\u{c4}pfel \u{df} \u{416}\u{443}\u{43a} \u{3c9}";
        let (pdf, _) = render(&format!("%%nxpdf 1\n{sample}")).unwrap();
        assert_structurally_valid(&pdf);
        let all = streams(&pdf);
        assert!(all[0].starts_with(&[0, 1, 0, 0]), "TrueType font file");
        let cmap = String::from_utf8(all[1].clone()).unwrap();
        assert!(cmap.contains("<0416>"), "Zhe maps back to U+0416");
        assert!(cmap.contains("<00C4>"));
        assert!(page_streams(&pdf)[0].contains(&hex(&font, sample)));
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains("/Subtype /Type0"));
        assert!(text.contains("/Encoding /Identity-H"));
        assert!(text.contains("/FontFile2 6 0 R"));
    }

    #[test]
    fn markup_headings_bold_italic_and_color() {
        let font = sans();
        let source = "%%nxpdf 1\n# Title\n\nPlain **bold** and *slanted* {#ff0000}red{/} end.";
        let (pdf, pages) = render(source).unwrap();
        assert_eq!(pages, 1);
        assert_structurally_valid(&pdf);
        let content = &page_streams(&pdf)[0];
        assert!(content.contains("/F1 20.00 Tf"), "heading size");
        assert!(content.contains(&hex(&font, "Title")));
        assert!(content.contains("2 Tr"), "bold uses fill+stroke");
        assert!(content.contains("1 0 0.20 1 "), "italic slant");
        assert!(content.contains("1.000 0.000 0.000 rg"), "red text");
        assert!(content.contains(&hex(&font, "bold")));
        assert!(!content.contains(&hex(&font, "**")));
    }

    #[test]
    fn markup_escapes_and_unknown_syntax_stay_literal() {
        let font = sans();
        let cells = parse_inline(r"\*a\* \{#fff} {#zzz} \x \#", false);
        let text: String = cells.iter().map(|c| c.ch).collect();
        assert_eq!(text, r"*a* {#fff} {#zzz} \x #");
        assert!(
            cells
                .iter()
                .all(|c| !c.bold && !c.italic && c.color.is_none())
        );
        let (pdf, _) = render("%%nxpdf 1\n\\# literal hash\n@unknown stays").unwrap();
        let content = &page_streams(&pdf)[0];
        assert!(content.contains(&hex(&font, "# literal hash @unknown stays")));
    }

    #[test]
    fn color_specs_parse_strictly() {
        assert_eq!(parse_color("#fff"), Some([255, 255, 255]));
        assert_eq!(parse_color("#0a0B0c"), Some([10, 11, 12]));
        assert_eq!(parse_color("fff"), None);
        assert_eq!(parse_color("#ffff"), None);
        assert_eq!(parse_color("#ggg"), None);
        assert_eq!(parse_color("#\u{e9}\u{e9}"), None);
    }

    #[test]
    fn explicit_page_breaks_split_pages_and_collapse_when_empty() {
        let source = "%%nxpdf 1\n@pagebreak\nOne\n@pagebreak\n@pagebreak\nTwo\n@pagebreak";
        let (pdf, pages) = render(source).unwrap();
        assert_eq!(pages, 2);
        assert_eq!(assert_structurally_valid(&pdf), 1 + 1 + 5 + 2 * 2 + 1);
        let font = sans();
        let content = page_streams(&pdf);
        assert!(content[0].contains(&hex(&font, "One")));
        assert!(!content[0].contains(&hex(&font, "Two")));
        assert!(content[1].contains(&hex(&font, "Two")));
    }

    #[test]
    fn headers_footers_and_page_numbers() {
        let font = sans();
        let source = "%%nxpdf 1\n@header Left | Center | **Right**\n\
                      @footer Seite {page} von {pages}\nA\n@pagebreak\nB";
        let (pdf, pages) = render(source).unwrap();
        assert_eq!(pages, 2);
        let content = page_streams(&pdf);
        assert!(content[0].contains(&hex(&font, "Seite 1 von 2")));
        assert!(content[1].contains(&hex(&font, "Seite 2 von 2")));
        for page in &content {
            assert!(page.contains(&hex(&font, "Left")));
            assert!(page.contains(&hex(&font, "Right")));
            assert!(page.contains("0.333 0.333 0.333 rg"), "default header gray");
        }
        let zones = header_zones(&font, "a|b|c", "1", "1").unwrap();
        assert!(zones[0].0 < zones[1].0 && zones[1].0 < zones[2].0);
        assert_eq!(zones[0].0, MARGIN);
        assert!(header_zones(&font, "a|b|c|d", "1", "1").is_err());
        assert_eq!(header_zones(&font, r"a\|b", "1", "1").unwrap().len(), 1);
        assert_eq!(
            render("%%nxpdf 1\n@footer a|b|c|d\nx"),
            Err("PDF header or footer has more than three zones")
        );
        let long = format!("%%nxpdf 1\n@header {}\nx", "h".repeat(MAX_HEADER_CHARS + 1));
        assert!(render(&long).is_err());
    }

    #[test]
    fn paragraphs_join_and_wrap_within_margins() {
        let font = sans();
        let words = "wort ".repeat(120);
        let doc = markup_document(&font, &format!("%%nxpdf 1\n{words}\nsecond line")).unwrap();
        let lines: Vec<&Line> = doc
            .items
            .iter()
            .map(|item| match item {
                Item::Line(line) => line,
                Item::PageBreak => panic!(),
            })
            .collect();
        assert!(lines.len() > 3);
        for line in &lines {
            assert!(cells_width(&font, &line.cells, line.size) <= CONTENT_WIDTH + 0.01);
            assert!(line.cells.first().is_none_or(|c| c.ch != ' '));
        }
        let last: String = lines.last().unwrap().cells.iter().map(|c| c.ch).collect();
        assert!(last.ends_with("wort second line"));
        assert!(lines[0].before > 0.0 && lines[1].before == 0.0);
    }

    #[test]
    fn long_markup_paginates_and_respects_hard_breaks() {
        let paragraphs: Vec<String> = (0..200).map(|n| format!("Absatz {n}")).collect();
        let source = format!("%%nxpdf 1\n{}", paragraphs.join("\n\n"));
        let (pdf, pages) = render(&source).unwrap();
        assert!(pages >= 3);
        assert_eq!(assert_structurally_valid(&pdf), 7 + pages * 2 + 1);
        let cells = parse_inline(r"a\nb", false);
        assert_eq!(wrap(&sans(), &cells, 11.0, 400.0, true).len(), 2);
    }

    #[test]
    fn markup_version_and_bom_handling() {
        assert_eq!(
            render("%%nxpdf 2\nx"),
            Err("unsupported NXPDF markup version")
        );
        let (pdf, _) = render("\u{feff}%%nxpdf 1\nx\r\n").unwrap();
        assert!(String::from_utf8_lossy(&pdf).contains("/BaseFont /UbuntuLight"));
        let (pdf, _) = render("not %%nxpdf 1\n").unwrap();
        assert!(String::from_utf8_lossy(&pdf).contains("/BaseFont /Hack-Regular"));
    }

    #[test]
    fn output_is_deterministic() {
        let source = "%%nxpdf 1\n# A\ntext *x*\n@footer {page}";
        assert_eq!(render(source).unwrap(), render(source).unwrap());
    }

    #[test]
    fn empty_document_still_has_one_page() {
        let (pdf, pages) = render("").unwrap();
        assert_eq!(pages, 1);
        assert_structurally_valid(&pdf);
        let (pdf, pages) = render("%%nxpdf 1").unwrap();
        assert_eq!(pages, 1);
        assert_structurally_valid(&pdf);
    }

    #[test]
    fn size_and_page_limits_fail_closed() {
        assert!(render(&"x".repeat(MAX_TEXT_BYTES + 1)).is_err());
        let many_lines = "\n".repeat(MAX_PAGES * 80);
        assert_eq!(
            render(&many_lines),
            Err("document exceeds the PDF page limit")
        );
        let markup = format!("%%nxpdf 1\n{}", "x\n@pagebreak\n".repeat(MAX_PAGES + 1));
        assert_eq!(render(&markup), Err("document exceeds the PDF page limit"));
        let project = TempProject::new("pdf-limit");
        assert!(export(&project.0, "o.pdf", &many_lines).is_err());
        assert!(!project.0.join("o.pdf").exists());
    }

    #[test]
    fn export_writes_the_file_and_checks_the_path() {
        let project = TempProject::new("pdf");
        let summary = export(&project.0, "doc.pdf", "text").unwrap();
        assert_eq!(summary.pages, 1);
        let written = fs::read(project.0.join("doc.pdf")).unwrap();
        assert_eq!(written.len() as u64, summary.bytes);
        assert_structurally_valid(&written);
        assert!(export(&project.0, "../doc.pdf", "text").is_err());
        assert!(export(&project.0, "doc.txt", "text").is_err());
    }
}
