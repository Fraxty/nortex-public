//! Schreibt das ausfüllbare Feedback-Formular als PDF mit AcroForm-Feldern.
//!
//! Das PDF wird ohne externe Bibliothek erzeugt: vier A4-Seiten, Helvetica (WinAnsi) für die
//! Beschriftung, Formularfelder als Widget-Annotationen. Feldwerte sind UTF-16BE kodiert, die
//! Darstellung übernimmt der Viewer (`NeedAppearances`).

use std::fmt::Write as _;

use crate::i18n::{Language, translate};

const PAGE_WIDTH: f32 = 595.0;
const PAGE_HEIGHT: f32 = 842.0;
const MARGIN: f32 = 50.0;
const BAND_HEIGHT: f32 = 62.0;
const FIELD_WIDTH: f32 = PAGE_WIDTH - 2.0 * MARGIN;
const LINE_HEIGHT: f32 = 13.0;

pub const FILE_NAME: &str = "NORTEX_Feedback_Form.pdf";

/// Feldname und Wert. Vorbelegung ist optional; fehlende Felder bleiben leer.
pub type FieldValues = [(&'static str, String)];

pub const KIND_OPTIONS: [&str; 4] = [
    "Fehler melden",
    "Verbesserung vorschlagen",
    "Dokumentationsfehler melden",
    "Allgemeines Feedback",
];

pub const CATEGORY_OPTIONS: [&str; 8] = [
    "Bug / Fehler",
    "Verbesserungsvorschlag",
    "Funktionswunsch",
    "Dokumentationsfehler",
    "Benutzeroberfläche",
    "Performance",
    "Sicherheit",
    "Sonstiges",
];

pub const FREQUENCY_OPTIONS: [&str; 5] = ["Immer", "Häufig", "Manchmal", "Selten", "Einmalig"];

enum Item {
    Heading(String),
    Note(String),
    Text {
        label: String,
        name: &'static str,
        lines: u32,
    },
    Choice {
        label: String,
        name: &'static str,
        options: Vec<String>,
    },
    Check {
        label: String,
        name: &'static str,
    },
}

fn pages(language: Language, contact: Option<&str>) -> Vec<Vec<Item>> {
    let text = |german: &'static str| translate(language, german).to_owned();
    let options = |list: &[&'static str]| list.iter().map(|option| text(option)).collect();
    let line = |label: &'static str, name| Item::Text {
        label: text(label),
        name,
        lines: 1,
    };
    let area = |label: &'static str, name, lines| Item::Text {
        label: text(label),
        name,
        lines,
    };

    let delivery = match contact {
        Some(address) => translate(
            language,
            "Senden Sie die gespeicherte Datei als E-Mail-Anhang an {}.",
        )
        .replace("{}", address),
        None => text("Hängen Sie die gespeicherte Datei an eine E-Mail an den Entwickler an."),
    };

    vec![
        vec![
            Item::Heading(text("Allgemeine Angaben")),
            Item::Note(text(
                "Füllen Sie das Formular am Rechner aus und speichern Sie es. Alle Angaben sind freiwillig, sofern Sie eine Rückmeldung senden möchten.",
            )),
            Item::Note(delivery),
            line("Name (optional)", "name"),
            line("E-Mail-Adresse (optional)", "reply_email"),
            line("NORTEX-Version", "version"),
            line("Betriebssystem", "os"),
            line("Datum", "date"),
            Item::Choice {
                label: text("Art der Rückmeldung"),
                name: "kind",
                options: options(&KIND_OPTIONS),
            },
            Item::Choice {
                label: text("Kategorie"),
                name: "category",
                options: options(&CATEGORY_OPTIONS),
            },
        ],
        vec![
            Item::Heading(text("Fehlerbericht")),
            line("Titel des Fehlers", "bug_title"),
            area("Beschreibung", "bug_description", 5),
            area("Schritte zur Reproduktion", "bug_steps", 6),
            area("Erwartetes Verhalten", "bug_expected", 3),
            area("Tatsächliches Verhalten", "bug_actual", 3),
            area("Fehlermeldungen", "bug_error", 4),
            Item::Choice {
                label: text("Häufigkeit des Auftretens"),
                name: "bug_frequency",
                options: options(&FREQUENCY_OPTIONS),
            },
            line("Betroffene Funktion oder Komponente", "bug_component"),
        ],
        vec![
            Item::Heading(text("Verbesserungsvorschläge")),
            line("Gewünschte Funktion", "feature_title"),
            area("Beschreibung", "feature_description", 6),
            area("Welches Problem soll gelöst werden?", "feature_problem", 5),
            area("Warum wäre diese Funktion sinnvoll?", "feature_reason", 5),
        ],
        vec![
            Item::Heading(text("Datenschutz und zusätzliche Informationen")),
            Item::Note(text(
                "Dieses Formular wird nur lokal auf Ihrem Gerät ausgefüllt. NORTEX überträgt nichts automatisch. Erst wenn Sie die Datei selbst per E-Mail senden, erhält der Empfänger Ihre Angaben.",
            )),
            Item::Note(text(
                "Die Angaben sollen ausschließlich zur Bearbeitung Ihrer Rückmeldung verwendet werden. Entfernen Sie vor dem Versand Passwörter, Zugangstoken und persönliche Daten aus allen Feldern.",
            )),
            Item::Check {
                label: text("Ich füge freiwillig technische Angaben bei."),
                name: "consent_technical",
            },
            area("Freiwillige technische Angaben", "tech_info", 5),
            line("Betroffene Extension", "tech_extension"),
            area("Weitere Informationen", "extra_info", 6),
            Item::Check {
                label: text("Ich bestätige, dass meine Angaben freiwillig erfolgen."),
                name: "consent_voluntary",
            },
        ],
    ]
}

struct Builder {
    objects: Vec<Vec<u8>>,
}

impl Builder {
    fn reserve(&mut self) -> usize {
        self.objects.push(Vec::new());
        self.objects.len()
    }

    fn set(&mut self, id: usize, body: Vec<u8>) {
        self.objects[id - 1] = body;
    }

    fn add(&mut self, body: Vec<u8>) -> usize {
        let id = self.reserve();
        self.set(id, body);
        id
    }

    fn add_stream(&mut self, dictionary: &str, data: &[u8]) -> usize {
        let mut body = format!("<< {dictionary} /Length {} >>\nstream\n", data.len()).into_bytes();
        body.extend_from_slice(data);
        body.extend_from_slice(b"\nendstream");
        self.add(body)
    }

    fn finish(self, root: usize, info: usize) -> Vec<u8> {
        let mut out = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec();
        let mut offsets = Vec::with_capacity(self.objects.len());
        for (index, body) in self.objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n", index + 1).as_bytes());
            out.extend_from_slice(body);
            out.extend_from_slice(b"\nendobj\n");
        }
        let xref = out.len();
        let mut table = format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1);
        for offset in offsets {
            let _ = writeln!(table, "{offset:010} 00000 n ");
        }
        let _ = write!(
            table,
            "trailer\n<< /Size {} /Root {root} 0 R /Info {info} 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            self.objects.len() + 1
        );
        out.extend_from_slice(table.as_bytes());
        out
    }
}

/// Baut das Formular. `values` belegt Felder vor, `contact` erscheint als Empfänger im Hinweistext.
pub fn build_form(language: Language, contact: Option<&str>, values: &FieldValues) -> Vec<u8> {
    let mut builder = Builder {
        objects: Vec::new(),
    };
    let catalog = builder.reserve();
    let page_tree = builder.reserve();
    let regular = builder.add(
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
            .to_vec(),
    );
    let bold = builder.add(
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>"
            .to_vec(),
    );
    let box_style = "/Type /XObject /Subtype /Form /BBox [0 0 12 12]";
    let check_off = builder.add_stream(box_style, b"");
    let check_on = builder.add_stream(
        box_style,
        b"0.09 0.14 0.24 RG 1.6 w 2.5 2.5 m 9.5 9.5 l S 2.5 9.5 m 9.5 2.5 l S",
    );

    let pages = pages(language, contact);
    let total = pages.len();
    let mut page_ids = Vec::new();
    let mut fields = Vec::new();
    for (index, items) in pages.iter().enumerate() {
        let page = builder.reserve();
        let mut sheet = Sheet::new(language, index + 1, total);
        let mut widgets = Vec::new();
        for item in items {
            sheet.place(
                item,
                values,
                page,
                &mut builder,
                (check_on, check_off),
                &mut widgets,
            );
        }
        let content = builder.add_stream("", &sheet.content);
        let annots: String = widgets.iter().map(|id| format!("{id} 0 R ")).collect();
        builder.set(
            page,
            format!(
                "<< /Type /Page /Parent {page_tree} 0 R /MediaBox [0 0 {PAGE_WIDTH} {PAGE_HEIGHT}] /Resources << /Font << /F1 {regular} 0 R /F2 {bold} 0 R >> >> /Contents {content} 0 R /Annots [ {annots}] >>"
            )
            .into_bytes(),
        );
        page_ids.push(page);
        fields.extend(widgets);
    }

    let kids: String = page_ids.iter().map(|id| format!("{id} 0 R ")).collect();
    builder.set(
        page_tree,
        format!("<< /Type /Pages /Kids [ {kids}] /Count {total} >>").into_bytes(),
    );
    let field_list: String = fields.iter().map(|id| format!("{id} 0 R ")).collect();
    builder.set(
        catalog,
        format!(
            "<< /Type /Catalog /Pages {page_tree} 0 R /Lang ({}) /AcroForm << /Fields [ {field_list}] /DR << /Font << /Helv {regular} 0 R >> >> /DA (/Helv 0 Tf 0 g) /NeedAppearances true >> >>",
            language.code()
        )
        .into_bytes(),
    );
    let info = builder.add(b"<< /Title (NORTEX Feedback Form) /Producer (NORTEX Code) >>".to_vec());
    builder.finish(catalog, info)
}

struct Sheet {
    content: Vec<u8>,
    y: f32,
}

impl Sheet {
    fn new(language: Language, number: usize, total: usize) -> Self {
        let mut sheet = Self {
            content: Vec::new(),
            y: PAGE_HEIGHT - BAND_HEIGHT - 34.0,
        };
        let title = translate(language, "Feedback-Formular");
        sheet.draw_header(title);
        let footer = format!("NORTEX  ·  {title}  ·  {number} / {total}");
        sheet.text("F1", 8.0, MARGIN, 28.0, "0.45 g", &footer);
        sheet
    }

    fn raw(&mut self, operators: &str) {
        self.content.extend_from_slice(operators.as_bytes());
        self.content.push(b'\n');
    }

    fn text(&mut self, font: &str, size: f32, x: f32, y: f32, color: &str, value: &str) {
        self.content.extend_from_slice(
            format!("{color} BT /{font} {size} Tf {x:.1} {y:.1} Td (").as_bytes(),
        );
        self.content.extend(win_ansi_escaped(value));
        self.raw(") Tj ET");
    }

    // Vereinfachte Vektorfassung des NORTEX-Symbols (zwei Balken und Diagonale).
    fn draw_header(&mut self, subtitle: &str) {
        self.raw(&format!(
            "0.09 0.14 0.24 rg 0 {:.1} {PAGE_WIDTH} {BAND_HEIGHT} re f",
            PAGE_HEIGHT - BAND_HEIGHT
        ));
        let scale = (BAND_HEIGHT * 0.62) / 994.0;
        let origin_y = PAGE_HEIGHT - BAND_HEIGHT * 0.81;
        let point = |x: f32, y: f32| {
            format!(
                "{:.2} {:.2}",
                MARGIN + (x - 598.0) * scale,
                origin_y + (1521.0 - y) * scale
            )
        };
        self.raw("1 1 1 rg");
        for polygon in [
            [
                (598.0, 527.0),
                (776.0, 527.0),
                (776.0, 1521.0),
                (598.0, 1521.0),
            ],
            [
                (1272.0, 527.0),
                (1450.0, 527.0),
                (1450.0, 1521.0),
                (1272.0, 1521.0),
            ],
            [
                (598.0, 527.0),
                (776.0, 527.0),
                (1450.0, 1521.0),
                (1272.0, 1521.0),
            ],
        ] {
            let mut path = format!("{} m", point(polygon[0].0, polygon[0].1));
            for (x, y) in &polygon[1..] {
                let _ = write!(path, " {} l", point(*x, *y));
            }
            self.raw(&format!("{path} h f"));
        }
        self.text(
            "F2",
            20.0,
            MARGIN + 42.0,
            PAGE_HEIGHT - 31.0,
            "1 g",
            "NORTEX",
        );
        self.text(
            "F1",
            10.0,
            MARGIN + 42.0,
            PAGE_HEIGHT - 48.0,
            "0.78 0.84 0.92 rg",
            subtitle,
        );
    }

    fn place(
        &mut self,
        item: &Item,
        values: &FieldValues,
        page: usize,
        builder: &mut Builder,
        check: (usize, usize),
        widgets: &mut Vec<usize>,
    ) {
        match item {
            Item::Heading(title) => {
                self.text("F2", 14.0, MARGIN, self.y, "0.09 0.14 0.24 rg", title);
                self.y -= 8.0;
                self.raw(&format!(
                    "0.75 0.80 0.86 RG 0.8 w {MARGIN} {:.1} m {} {:.1} l S",
                    self.y,
                    PAGE_WIDTH - MARGIN,
                    self.y
                ));
                self.y -= 18.0;
            }
            Item::Note(paragraph) => {
                for line in wrap(paragraph, 100) {
                    self.text("F1", 9.0, MARGIN, self.y, "0.30 0.34 0.40 rg", &line);
                    self.y -= 12.0;
                }
                self.y -= 6.0;
            }
            Item::Text { label, name, lines } => {
                let height = *lines as f32 * LINE_HEIGHT + 7.0;
                let rect = self.field_box(label, height);
                let flags = if *lines > 1 { 4096 } else { 0 };
                let value = lookup(values, name)
                    .map(|value| format!(" /V {}", utf16_hex(value)))
                    .unwrap_or_default();
                let dictionary = format!("/FT /Tx /Ff {flags} /DA (/Helv 10 Tf 0 g){value}");
                widgets.push(builder.add(widget(name, label, rect, page, &dictionary)));
            }
            Item::Choice {
                label,
                name,
                options,
            } => {
                let rect = self.field_box(label, 20.0);
                let list: String = options
                    .iter()
                    .map(|option| utf16_hex(option) + " ")
                    .collect();
                let value = lookup(values, name)
                    .filter(|value| options.iter().any(|option| option == value))
                    .map(|value| format!(" /V {}", utf16_hex(value)))
                    .unwrap_or_default();
                let dictionary =
                    format!("/FT /Ch /Ff 131072 /DA (/Helv 10 Tf 0 g) /Opt [ {list}]{value}");
                widgets.push(builder.add(widget(name, label, rect, page, &dictionary)));
            }
            Item::Check { label, name } => {
                let top = self.y + 3.0;
                let rect = [MARGIN, top - 12.0, MARGIN + 12.0, top];
                self.text("F1", 9.5, MARGIN + 20.0, self.y - 6.0, "0 g", label);
                let state = if lookup(values, name) == Some("1") {
                    "Yes"
                } else {
                    "Off"
                };
                let dictionary = format!(
                    "/FT /Btn /Ff 0 /V /{state} /AS /{state} /AP << /N << /Yes {} 0 R /Off {} 0 R >> >>",
                    check.0, check.1
                );
                widgets.push(builder.add(widget(name, label, rect, page, &dictionary)));
                self.y -= 26.0;
            }
        }
    }

    fn field_box(&mut self, label: &str, height: f32) -> [f32; 4] {
        self.text("F2", 9.0, MARGIN, self.y, "0.09 0.14 0.24 rg", label);
        let top = self.y - 5.0;
        let bottom = top - height;
        self.y = bottom - 14.0;
        [MARGIN, bottom, MARGIN + FIELD_WIDTH, top]
    }
}

fn widget(name: &str, tooltip: &str, rect: [f32; 4], page: usize, dictionary: &str) -> Vec<u8> {
    format!(
        "<< /Type /Annot /Subtype /Widget /T ({name}) /TU {} /Rect [{:.1} {:.1} {:.1} {:.1}] /P {page} 0 R /F 4 /BS << /W 1 /S /S >> /MK << /BC [0.55 0.60 0.68] /BG [1 1 1] >> {dictionary} >>",
        utf16_hex(tooltip),
        rect[0],
        rect[1],
        rect[2],
        rect[3]
    )
    .into_bytes()
}

fn lookup<'a>(values: &'a FieldValues, name: &str) -> Option<&'a str> {
    values
        .iter()
        .find(|(field, _)| *field == name)
        .map(|(_, value)| value.as_str())
}

/// Zeilenumbruch nach Wortgrenzen; die Breite wird über die Zeichenzahl geschätzt.
fn wrap(paragraph: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in paragraph.split_whitespace() {
        if !current.is_empty() && current.chars().count() + 1 + word.chars().count() > max_chars {
            lines.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// UTF-16BE-Hexstring mit Byte-Order-Mark; Steuerzeichen außer Zeilenumbruch entfallen.
fn utf16_hex(value: &str) -> String {
    let mut out = String::from("<FEFF");
    for character in value.chars().filter(|c| !c.is_control() || *c == '\n') {
        let mut buffer = [0u16; 2];
        for unit in character.encode_utf16(&mut buffer) {
            let unit = if character == '\n' { 0x000D } else { *unit };
            let _ = write!(out, "{unit:04X}");
        }
    }
    out.push('>');
    out
}

fn win_ansi_escaped(value: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(value.len());
    for character in value.chars() {
        let byte = match character {
            '\u{20}'..='\u{7E}' | '\u{A0}'..='\u{FF}' => character as u8,
            '–' => 0x96,
            '—' => 0x97,
            '„' => 0x84,
            '“' => 0x93,
            '”' => 0x94,
            '‚' => 0x82,
            '‘' => 0x91,
            '’' => 0x92,
            '•' => 0x95,
            '€' => 0x80,
            _ => b'?',
        };
        if matches!(byte, b'(' | b')' | b'\\') {
            out.push(b'\\');
        }
        out.push(byte);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(haystack: &[u8], needle: &str) -> usize {
        let needle = needle.as_bytes();
        haystack
            .windows(needle.len())
            .filter(|window| *window == needle)
            .count()
    }

    #[test]
    fn form_has_four_pages_and_interactive_fields() {
        let pdf = build_form(Language::German, None, &[]);
        assert!(pdf.starts_with(b"%PDF-1.7"));
        assert_eq!(count(&pdf, "/Type /Page "), 4);
        assert!(count(&pdf, "/FT /Tx") >= 18);
        assert!(count(&pdf, "/FT /Ch") >= 3);
        assert_eq!(count(&pdf, "/FT /Btn"), 2);
        assert_eq!(count(&pdf, "/AcroForm"), 1);
    }

    #[test]
    fn cross_reference_offsets_point_to_their_objects() {
        let pdf = build_form(Language::English, Some("a@b.example"), &[]);
        let start = pdf.windows(9).rposition(|w| w == b"startxref").unwrap();
        let tail = String::from_utf8_lossy(&pdf[start..]).into_owned();
        let table_offset: usize = tail.lines().nth(1).unwrap().trim().parse().unwrap();
        let table = String::from_utf8_lossy(&pdf[table_offset..]).into_owned();
        let entries: Vec<&str> = table
            .lines()
            .skip(3)
            .take_while(|line| line.ends_with(" n "))
            .collect();
        assert!(entries.len() > 20);
        for (index, entry) in entries.iter().enumerate() {
            let offset: usize = entry[..10].parse().unwrap();
            let expected = format!("{} 0 obj", index + 1);
            assert!(
                pdf[offset..].starts_with(expected.as_bytes()),
                "object {}",
                index + 1
            );
        }
    }

    #[test]
    fn values_are_stored_utf16_and_special_characters_survive() {
        let values = [("bug_title", "Ä (x) \\ \"q\" 日本\n2".to_owned())];
        let pdf = build_form(Language::German, None, &values);
        assert_eq!(count(&pdf, "/V <FEFF00C4"), 1);
        assert_eq!(count(&pdf, "65E5"), 1);
        assert_eq!(count(&pdf, "000D0032>"), 1);
    }

    #[test]
    fn choice_value_outside_the_options_is_ignored() {
        let values = [("kind", "frei erfunden".to_owned())];
        let pdf = build_form(Language::German, None, &values);
        assert_eq!(count(&pdf, "/V <"), 0);
    }

    #[test]
    fn checked_box_uses_the_yes_state() {
        let values = [("consent_technical", "1".to_owned())];
        let pdf = build_form(Language::German, None, &values);
        assert_eq!(count(&pdf, "/V /Yes /AS /Yes"), 1);
        assert_eq!(count(&pdf, "/V /Off /AS /Off"), 1);
    }

    #[test]
    fn english_form_is_translated() {
        let german = build_form(Language::German, None, &[]);
        let english = build_form(Language::English, None, &[]);
        assert_eq!(count(&german, "(Fehlerbericht)"), 1);
        assert_eq!(count(&english, "(Fehlerbericht)"), 0);
        assert_eq!(count(&english, "(Bug report)"), 1);
    }

    #[test]
    fn every_pdf_text_has_an_english_entry() {
        const SAME_IN_ENGLISH: [&str; 2] = ["Name (optional)", "Performance"];
        let source: &'static str = include_str!("pdf.rs");
        let production = source.split("#[cfg(test)]").next().unwrap();
        let mut missing = Vec::new();
        for marker in ["(text(\"", " text(\"", " line(\"", " area(\""] {
            for part in production.split(marker).skip(1) {
                let literal = part.split('"').next().unwrap();
                if translate(Language::English, literal) == literal
                    && !SAME_IN_ENGLISH.contains(&literal)
                {
                    missing.push(literal);
                }
            }
        }
        let delivery = "Senden Sie die gespeicherte Datei als E-Mail-Anhang an {}.";
        if translate(Language::English, delivery) == delivery {
            missing.push(delivery);
        }
        for option in KIND_OPTIONS
            .iter()
            .chain(&CATEGORY_OPTIONS)
            .chain(&FREQUENCY_OPTIONS)
        {
            if translate(Language::English, option) == *option && !SAME_IN_ENGLISH.contains(option)
            {
                missing.push(option);
            }
        }
        assert!(missing.is_empty(), "{missing:?}");
    }
}
