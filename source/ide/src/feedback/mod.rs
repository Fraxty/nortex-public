//! Feedback-Formular: Eingaben, Prüfung, Schutz vor Geheimnissen, E-Mail-Link und lokaler Entwurf.
//!
//! Nichts in diesem Modul sendet Daten. Der Nutzer exportiert ein PDF und schickt es selbst ab.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::i18n::{Language, translate};

pub mod pdf;

/// Zentrale Empfängeradresse für Rückmeldungen. Steht hier `None`, bleiben die
/// E-Mail-Schaltflächen deaktiviert.
pub const CONTACT_EMAIL: Option<&str> = Some("NorisTec@protonmail.com");

const MAX_TITLE_CHARS: usize = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Kind {
    #[default]
    Bug,
    Improvement,
    Documentation,
    General,
}

impl Kind {
    pub const ALL: [Kind; 4] = [
        Kind::Bug,
        Kind::Improvement,
        Kind::Documentation,
        Kind::General,
    ];

    pub fn label(self) -> &'static str {
        pdf::KIND_OPTIONS[self as usize]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Category {
    #[default]
    Bug,
    Improvement,
    FeatureRequest,
    Documentation,
    UserInterface,
    Performance,
    Security,
    Other,
}

impl Category {
    pub const ALL: [Category; 8] = [
        Category::Bug,
        Category::Improvement,
        Category::FeatureRequest,
        Category::Documentation,
        Category::UserInterface,
        Category::Performance,
        Category::Security,
        Category::Other,
    ];

    pub fn label(self) -> &'static str {
        pdf::CATEGORY_OPTIONS[self as usize]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Frequency {
    Always,
    Often,
    #[default]
    Sometimes,
    Rarely,
    Once,
}

impl Frequency {
    pub const ALL: [Frequency; 5] = [
        Frequency::Always,
        Frequency::Often,
        Frequency::Sometimes,
        Frequency::Rarely,
        Frequency::Once,
    ];

    pub fn label(self) -> &'static str {
        pdf::FREQUENCY_OPTIONS[self as usize]
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FeedbackForm {
    pub kind: Kind,
    pub category: Category,
    pub title: String,
    pub description: String,
    pub contact_email: String,
    pub steps: String,
    pub expected: String,
    pub actual: String,
    pub frequency: Frequency,
    pub error_message: String,
    pub component: String,
    pub include_technical: bool,
    pub extension: String,
    pub log_excerpt: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    TitleMissing,
    TitleTooLong,
    DescriptionMissing,
    StepsMissing,
    ActualMissing,
    EmailInvalid,
}

impl Problem {
    pub fn message(self) -> &'static str {
        match self {
            Self::TitleMissing => "Bitte geben Sie einen Titel an.",
            Self::TitleTooLong => "Der Titel ist zu lang (höchstens 120 Zeichen).",
            Self::DescriptionMissing => "Bitte beschreiben Sie Ihr Anliegen.",
            Self::StepsMissing => "Bitte nennen Sie die Schritte zur Reproduktion.",
            Self::ActualMissing => "Bitte beschreiben Sie das tatsächliche Verhalten.",
            Self::EmailInvalid => "Die Kontakt-E-Mail-Adresse ist ungültig.",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretKind {
    Credential,
    Email,
    UserPath,
}

impl SecretKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Credential => "mögliches Passwort oder Token",
            Self::Email => "E-Mail-Adresse",
            Self::UserPath => "Benutzername im Dateipfad",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub field: &'static str,
    pub kind: SecretKind,
}

struct Detector {
    kind: SecretKind,
    pattern: Regex,
    replacement: &'static str,
}

fn detectors() -> &'static [Detector] {
    static DETECTORS: OnceLock<Vec<Detector>> = OnceLock::new();
    DETECTORS.get_or_init(|| {
        let credential = |pattern: &str| Detector {
            kind: SecretKind::Credential,
            pattern: Regex::new(pattern).expect("valid pattern"),
            replacement: "[entfernt]",
        };
        vec![
            credential(concat!(
                "-----BEGIN ",
                "[A-Z ]*PRIVATE KEY",
                "-----[\\s\\S]*?(-----END ",
                "[A-Z ]*PRIVATE KEY",
                "-----|$)"
            )),
            credential(
                r#"(?i)\b(password|passwort|passwd|secret|token|api[_-]?key|authorization)\b["']?\s*[:=]\s*["']?[^\s"',;]{4,}"#,
            ),
            credential(r"(?i)\bbearer\s+[A-Za-z0-9._~+/=-]{16,}"),
            credential(r"\b(ghp|gho|ghs|github_pat|sk|xox[bp])[_-][A-Za-z0-9_-]{16,}"),
            credential(r"\bAKIA[0-9A-Z]{16}\b"),
            credential(r"\b[A-Za-z0-9+/_-]{40,}={0,2}"),
            Detector {
                kind: SecretKind::Email,
                pattern: Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}")
                    .expect("valid pattern"),
                replacement: "[E-Mail entfernt]",
            },
            Detector {
                kind: SecretKind::UserPath,
                pattern: Regex::new(r"(?i)(?:[A-Z]:\\Users\\|/home/|/Users/)[^\\/\s]+")
                    .expect("valid pattern"),
                replacement: "<Benutzerpfad>",
            },
        ]
    })
}

fn has_finding(text: &str) -> Vec<SecretKind> {
    detectors()
        .iter()
        .filter(|detector| detector.pattern.is_match(text))
        .map(|detector| detector.kind)
        .collect()
}

/// Ersetzt erkannte Geheimnisse, Adressen und Benutzerpfade durch Platzhalter.
pub fn redact(text: &str) -> String {
    detectors().iter().fold(text.to_owned(), |text, detector| {
        detector
            .pattern
            .replace_all(&text, detector.replacement)
            .into_owned()
    })
}

pub fn is_valid_email(address: &str) -> bool {
    let Some((local, domain)) = address.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && !address.chars().any(|c| c.is_whitespace() || c.is_control())
        && !domain.contains('@')
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
}

impl FeedbackForm {
    pub fn validate(&self) -> Vec<Problem> {
        let mut problems = Vec::new();
        let title = self.title.trim();
        if title.is_empty() {
            problems.push(Problem::TitleMissing);
        } else if title.chars().count() > MAX_TITLE_CHARS {
            problems.push(Problem::TitleTooLong);
        }
        if self.description.trim().is_empty() {
            problems.push(Problem::DescriptionMissing);
        }
        if self.kind == Kind::Bug {
            if self.steps.trim().is_empty() {
                problems.push(Problem::StepsMissing);
            }
            if self.actual.trim().is_empty() {
                problems.push(Problem::ActualMissing);
            }
        }
        let email = self.contact_email.trim();
        if !email.is_empty() && !is_valid_email(email) {
            problems.push(Problem::EmailInvalid);
        }
        problems
    }

    /// Freitextfelder, die in einen Export gelangen können. Die Kontaktadresse ist ausgenommen,
    /// weil der Nutzer sie bewusst angibt.
    fn exported_fields(&self) -> Vec<(&'static str, &str)> {
        let mut fields = vec![
            ("Titel", self.title.as_str()),
            ("Beschreibung", self.description.as_str()),
        ];
        if self.kind == Kind::Bug {
            fields.extend([
                ("Schritte zur Reproduktion", self.steps.as_str()),
                ("Erwartetes Verhalten", self.expected.as_str()),
                ("Tatsächliches Verhalten", self.actual.as_str()),
                ("Fehlermeldung", self.error_message.as_str()),
                (
                    "Betroffene Funktion oder Komponente",
                    self.component.as_str(),
                ),
            ]);
        }
        if self.include_technical {
            fields.extend([
                ("Betroffene Extension", self.extension.as_str()),
                ("Relevante Fehlermeldungen", self.log_excerpt.as_str()),
            ]);
        }
        fields
    }

    pub fn scan_for_secrets(&self) -> Vec<Finding> {
        self.exported_fields()
            .into_iter()
            .flat_map(|(field, text)| {
                has_finding(text)
                    .into_iter()
                    .map(move |kind| Finding { field, kind })
            })
            .collect()
    }

    pub fn redact_all(&mut self) {
        for text in [
            &mut self.title,
            &mut self.description,
            &mut self.steps,
            &mut self.expected,
            &mut self.actual,
            &mut self.error_message,
            &mut self.component,
            &mut self.extension,
            &mut self.log_excerpt,
        ] {
            *text = redact(text);
        }
    }

    /// Technische Angaben, die nur bei ausdrücklicher Zustimmung ausgegeben werden.
    pub fn technical_info(&self, language: Language) -> Option<String> {
        if !self.include_technical {
            return None;
        }
        let mut info = technical_summary(language);
        if !self.extension.trim().is_empty() {
            info.push_str(&format!(
                "\n{}: {}",
                translate(language, "Betroffene Extension"),
                self.extension.trim()
            ));
        }
        if !self.log_excerpt.trim().is_empty() {
            info.push_str(&format!(
                "\n{}:\n{}",
                translate(language, "Relevante Fehlermeldungen"),
                self.log_excerpt.trim()
            ));
        }
        Some(info)
    }

    /// Werte für die Vorbelegung des PDF-Formulars.
    pub fn pdf_values(&self, language: Language) -> Vec<(&'static str, String)> {
        let label = |german: &'static str| translate(language, german).to_owned();
        let mut values = vec![
            ("kind", label(self.kind.label())),
            ("category", label(self.category.label())),
            ("reply_email", self.contact_email.trim().to_owned()),
        ];
        if self.kind == Kind::Bug {
            values.extend([
                ("bug_title", self.title.trim().to_owned()),
                ("bug_description", self.description.trim().to_owned()),
                ("bug_steps", self.steps.trim().to_owned()),
                ("bug_expected", self.expected.trim().to_owned()),
                ("bug_actual", self.actual.trim().to_owned()),
                ("bug_error", self.error_message.trim().to_owned()),
                ("bug_frequency", label(self.frequency.label())),
                ("bug_component", self.component.trim().to_owned()),
            ]);
        } else {
            values.extend([
                ("feature_title", self.title.trim().to_owned()),
                ("feature_description", self.description.trim().to_owned()),
            ]);
        }
        if let Some(info) = self.technical_info(language) {
            values.extend([
                ("version", env!("CARGO_PKG_VERSION").to_owned()),
                (
                    "os",
                    format!("{} ({})", std::env::consts::OS, std::env::consts::ARCH),
                ),
                ("tech_info", info),
                ("tech_extension", self.extension.trim().to_owned()),
                ("consent_technical", "1".to_owned()),
            ]);
        }
        values.retain(|(_, value)| !value.is_empty());
        values
    }

    pub fn save_draft(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        std::fs::write(path, json)
    }

    pub fn load_draft(path: &Path) -> io::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        serde_json::from_str(&json)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }
}

pub fn technical_summary(language: Language) -> String {
    let line =
        |label: &'static str, value: &str| format!("{}: {value}", translate(language, label));
    [
        line("NORTEX-Version", env!("CARGO_PKG_VERSION")),
        line("Betriebssystem", std::env::consts::OS),
        line("Systemarchitektur", std::env::consts::ARCH),
        line("Compiler-Version", nortex_compiler::VERSION),
    ]
    .join("\n")
}

/// `%APPDATA%\NORTEX Code\feedback-draft.json`, wie die Einstellungen.
pub fn draft_path() -> PathBuf {
    crate::ui::settings::settings_path().with_file_name("feedback-draft.json")
}

pub fn mail_subject(language: Language) -> &'static str {
    translate(language, "NORTEX – Feedback / Fehlerbericht")
}

pub fn mail_body(language: Language) -> String {
    translate(
        language,
        "Hallo,\n\nich sende eine Rückmeldung zu NORTEX. Das ausgefüllte Formular hänge ich als PDF an diese E-Mail an.\n\nViele Grüße",
    )
    .to_owned()
}

pub fn mailto_url(recipient: &str, subject: &str, body: &str) -> String {
    format!(
        "mailto:{recipient}?subject={}&body={}",
        percent_encode(subject),
        percent_encode(body)
    )
}

fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for byte in text.replace('\n', "\r\n").bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Öffnet den Standard-E-Mail-Client. `NotFound` bedeutet: kein Programm für `mailto:` registriert.
pub fn open_mail_client(url: &str) -> io::Result<()> {
    #[cfg(windows)]
    {
        let registered = Command::new("reg")
            .args(["query", r"HKCR\mailto\shell\open\command"])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);
        if !registered {
            return Err(io::Error::new(io::ErrorKind::NotFound, "no mailto handler"));
        }
        Command::new("explorer").arg(url).spawn().map(drop)
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(url).spawn().map(drop)
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open").arg(url).spawn().map(drop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bug() -> FeedbackForm {
        FeedbackForm {
            title: "Absturz beim Speichern".into(),
            description: "Der Editor schließt sich.".into(),
            steps: "1. Datei öffnen\n2. Speichern".into(),
            actual: "Absturz".into(),
            ..FeedbackForm::default()
        }
    }

    #[test]
    fn complete_bug_report_is_valid() {
        assert!(bug().validate().is_empty());
    }

    #[test]
    fn required_fields_are_reported() {
        let problems = FeedbackForm::default().validate();
        assert_eq!(
            problems,
            [
                Problem::TitleMissing,
                Problem::DescriptionMissing,
                Problem::StepsMissing,
                Problem::ActualMissing
            ]
        );
    }

    #[test]
    fn suggestions_do_not_need_reproduction_steps() {
        let form = FeedbackForm {
            kind: Kind::Improvement,
            ..bug()
        };
        assert!(
            FeedbackForm {
                steps: String::new(),
                actual: String::new(),
                ..form
            }
            .validate()
            .is_empty()
        );
    }

    #[test]
    fn whitespace_only_and_overlong_titles_fail() {
        let blank = FeedbackForm {
            title: "  \n".into(),
            ..bug()
        };
        assert_eq!(blank.validate(), [Problem::TitleMissing]);
        let long = FeedbackForm {
            title: "ä".repeat(121),
            ..bug()
        };
        assert_eq!(long.validate(), [Problem::TitleTooLong]);
    }

    #[test]
    fn email_validation() {
        for good in ["a@b.de", "vorname.nachname+tag@sub.example.org"] {
            assert!(is_valid_email(good), "{good}");
        }
        for bad in [
            "", "a", "a@", "@b.de", "a@b", "a b@c.de", "a@@b.de", "a@b.", "a@.b", "a@b.de\n",
        ] {
            assert!(!is_valid_email(bad), "{bad:?}");
        }
        let form = FeedbackForm {
            contact_email: "kaputt".into(),
            ..bug()
        };
        assert_eq!(form.validate(), [Problem::EmailInvalid]);
    }

    #[test]
    fn secrets_emails_and_user_paths_are_found() {
        let token = format!("{}{}{}", "ghp", "_", "a".repeat(32));
        let form = FeedbackForm {
            error_message: "password=hunter22 und Bearer abcdefghijklmnopqrstuvwxyz".into(),
            description: "Mail an max@example.com, Datei C:\\Users\\Max\\proj\\a.nx".into(),
            actual: token,
            ..bug()
        };
        let kinds: Vec<_> = form
            .scan_for_secrets()
            .into_iter()
            .map(|f| (f.field, f.kind))
            .collect();
        assert!(kinds.contains(&("Fehlermeldung", SecretKind::Credential)));
        assert!(kinds.contains(&("Beschreibung", SecretKind::Email)));
        assert!(kinds.contains(&("Beschreibung", SecretKind::UserPath)));
        assert!(kinds.contains(&("Tatsächliches Verhalten", SecretKind::Credential)));
    }

    #[test]
    fn ordinary_text_has_no_findings() {
        assert!(bug().scan_for_secrets().is_empty());
        let form = FeedbackForm {
            description: "Der Token-Typ passt nicht zum Parser.".into(),
            ..bug()
        };
        assert!(form.scan_for_secrets().is_empty());
    }

    #[test]
    fn redaction_removes_the_values() {
        let text = "token: abc12345 pfad C:\\Users\\Max\\x mail max@example.com";
        let cleaned = redact(text);
        assert!(!cleaned.contains("abc12345"));
        assert!(!cleaned.contains("Max"));
        assert!(!cleaned.contains("max@example.com"));
        let mut form = FeedbackForm {
            description: text.into(),
            ..bug()
        };
        form.redact_all();
        assert!(form.scan_for_secrets().is_empty());
    }

    #[test]
    fn technical_info_needs_consent() {
        let mut form = FeedbackForm {
            log_excerpt: "Fehler 5".into(),
            ..bug()
        };
        assert!(form.technical_info(Language::German).is_none());
        assert!(
            form.pdf_values(Language::German)
                .iter()
                .all(|(name, _)| !matches!(
                    *name,
                    "version" | "os" | "tech_info" | "consent_technical"
                ))
        );
        form.include_technical = true;
        let info = form.technical_info(Language::English).unwrap();
        assert!(info.contains(env!("CARGO_PKG_VERSION")));
        assert!(info.contains("Compiler version"));
        assert!(info.contains("Fehler 5"));
        assert!(
            form.pdf_values(Language::English)
                .iter()
                .any(|(name, value)| *name == "consent_technical" && value == "1")
        );
    }

    #[test]
    fn mailto_encodes_special_characters() {
        let url = mailto_url("a@b.example", "Ä & ? =", "Zeile 1\nZeile 2 100%");
        assert_eq!(
            url,
            "mailto:a@b.example?subject=%C3%84%20%26%20%3F%20%3D&body=Zeile%201%0D%0AZeile%202%20100%25"
        );
        assert!(!url.contains(' ') && !url.contains('\n'));
    }

    #[test]
    fn draft_round_trips_and_rejects_garbage() {
        let path =
            std::env::temp_dir().join(format!("nortex-feedback-{}.json", std::process::id()));
        let form = FeedbackForm {
            title: "Ä \"q\" \\ 日本".into(),
            include_technical: true,
            ..bug()
        };
        form.save_draft(&path).unwrap();
        assert_eq!(FeedbackForm::load_draft(&path).unwrap(), form);
        std::fs::write(&path, "{kaputt").unwrap();
        assert_eq!(
            FeedbackForm::load_draft(&path).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        let _ = std::fs::remove_file(&path);
        assert_eq!(
            FeedbackForm::load_draft(&path).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
    }

    #[test]
    fn every_label_has_an_english_translation() {
        let labels = Kind::ALL
            .iter()
            .map(|k| k.label())
            .chain(Category::ALL.iter().map(|c| c.label()))
            .chain(Frequency::ALL.iter().map(|f| f.label()))
            .chain(
                [
                    Problem::TitleMissing,
                    Problem::TitleTooLong,
                    Problem::DescriptionMissing,
                    Problem::StepsMissing,
                    Problem::ActualMissing,
                    Problem::EmailInvalid,
                ]
                .iter()
                .map(|p| p.message()),
            )
            .chain(
                [
                    SecretKind::Credential,
                    SecretKind::Email,
                    SecretKind::UserPath,
                ]
                .iter()
                .map(|s| s.label()),
            );
        for label in labels {
            if label != "Performance" {
                assert_ne!(translate(Language::English, label), label, "{label}");
            }
        }
        assert_ne!(
            mail_subject(Language::English),
            mail_subject(Language::German)
        );
        assert_ne!(mail_body(Language::English), mail_body(Language::German));
    }
}
