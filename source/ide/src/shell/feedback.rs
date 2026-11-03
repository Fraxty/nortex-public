//! Hilfe- und Feedback-Fenster: Formular, PDF-Export, E-Mail-Kontakt und Datenschutzhinweise.
//!
//! Die Formularlogik liegt in `nortex_ide::feedback`, die PDF-Erzeugung in `feedback::pdf`.
//! Dieses Modul zeichnet nur die Oberfläche und führt die vom Nutzer ausgelösten Aktionen aus.

use crate::*;
use nortex_ide::feedback::{self, CONTACT_EMAIL, Category, FeedbackForm, Frequency, Kind, pdf};
use nortex_ide::i18n::{language, tr_fmt};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum HelpSection {
    #[default]
    Help,
    Report(Kind),
    Support,
}

const SECTIONS: [HelpSection; 6] = [
    HelpSection::Help,
    HelpSection::Report(Kind::Bug),
    HelpSection::Report(Kind::Improvement),
    HelpSection::Report(Kind::Documentation),
    HelpSection::Report(Kind::General),
    HelpSection::Support,
];

impl HelpSection {
    fn label(self) -> &'static str {
        match self {
            Self::Help => "Hilfe und Dokumentation",
            Self::Report(kind) => kind.label(),
            Self::Support => "Support und Kontakt",
        }
    }
}

#[derive(Default)]
pub(crate) struct FeedbackState {
    open: bool,
    section: HelpSection,
    form: FeedbackForm,
    accept_findings: bool,
    show_problems: bool,
    notice: Option<(bool, String)>,
}

impl FeedbackState {
    pub(crate) fn open_section(&mut self, section: HelpSection) {
        if self.section != section {
            self.notice = None;
            self.show_problems = false;
        }
        if let HelpSection::Report(kind) = section {
            self.form.kind = kind;
            self.form.category = match kind {
                Kind::Bug => Category::Bug,
                Kind::Improvement => Category::Improvement,
                Kind::Documentation => Category::Documentation,
                Kind::General => Category::Other,
            };
        }
        self.section = section;
        self.open = true;
    }

    fn succeed(&mut self, message: String) {
        self.notice = Some((true, message));
    }

    fn fail(&mut self, message: String) {
        self.notice = Some((false, message));
    }

    /// Prüft Pflichtfelder und vertrauliche Inhalte, bevor Formulardaten das Formular verlassen.
    fn form_is_ready(&mut self) -> bool {
        self.show_problems = true;
        let problems = self.form.validate();
        if !problems.is_empty() {
            let lines: Vec<&str> = problems.iter().map(|p| tr(p.message())).collect();
            self.fail(lines.join("\n"));
            return false;
        }
        if !self.form.scan_for_secrets().is_empty() && !self.accept_findings {
            self.fail(
                tr("Im Formular wurden mögliche vertrauliche Angaben gefunden. Schwärzen Sie sie oder bestätigen Sie ausdrücklich, dass sie bleiben sollen.")
                    .into(),
            );
            return false;
        }
        true
    }

    fn export_pdf(&mut self, with_form_data: bool) {
        if with_form_data && !self.form_is_ready() {
            return;
        }
        let values = if with_form_data {
            self.form.pdf_values(language())
        } else {
            Vec::new()
        };
        let bytes = pdf::build_form(language(), CONTACT_EMAIL, &values);
        let Some(path) = rfd::FileDialog::new()
            .set_file_name(pdf::FILE_NAME)
            .add_filter("PDF", &["pdf"])
            .save_file()
        else {
            return;
        };
        match std::fs::write(&path, bytes) {
            Ok(()) => self.succeed(tr_fmt(
                "PDF gespeichert: {}. Es wurde nichts gesendet.",
                &[path.display().to_string()],
            )),
            Err(error) => self.fail(tr_fmt(
                "PDF konnte nicht gespeichert werden: {}",
                &[error.to_string()],
            )),
        }
    }

    fn open_mail(&mut self, with_text: bool) {
        let Some(recipient) = CONTACT_EMAIL else {
            self.fail(tr("Es ist keine Kontaktadresse konfiguriert, der E-Mail-Versand ist deshalb nicht verfügbar. Sie können das PDF trotzdem speichern.").into());
            return;
        };
        let url = if with_text {
            let language = language();
            feedback::mailto_url(
                recipient,
                feedback::mail_subject(language),
                &feedback::mail_body(language),
            )
        } else {
            format!("mailto:{recipient}")
        };
        match feedback::open_mail_client(&url) {
            Ok(()) => self.succeed(
                tr("E-Mail-Programm geöffnet. Hängen Sie die gespeicherte PDF-Datei selbst an und bestätigen Sie den Versand dort. NORTEX sendet nichts.")
                    .into(),
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => self.fail(tr_fmt(
                "Kein E-Mail-Programm gefunden. Kopieren Sie die Adresse {} und senden Sie die PDF-Datei mit einem Webmail-Dienst oder einem anderen Gerät.",
                &[recipient.to_owned()],
            )),
            Err(error) => self.fail(tr_fmt(
                "E-Mail-Programm konnte nicht geöffnet werden: {}",
                &[error.to_string()],
            )),
        }
    }

    fn save_draft(&mut self) {
        let path = feedback::draft_path();
        match self.form.save_draft(&path) {
            Ok(()) => self.succeed(tr_fmt(
                "Entwurf lokal gespeichert: {}",
                &[path.display().to_string()],
            )),
            Err(error) => self.fail(tr_fmt(
                "Entwurf konnte nicht gespeichert werden: {}",
                &[error.to_string()],
            )),
        }
    }

    fn load_draft(&mut self) {
        match FeedbackForm::load_draft(&feedback::draft_path()) {
            Ok(form) => {
                self.form = form;
                self.succeed(tr("Entwurf geladen.").into());
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.fail(tr("Es ist kein gespeicherter Entwurf vorhanden.").into());
            }
            Err(error) => self.fail(tr_fmt(
                "Entwurf konnte nicht geladen werden: {}",
                &[error.to_string()],
            )),
        }
    }

    fn delete_draft(&mut self) {
        match std::fs::remove_file(feedback::draft_path()) {
            Ok(()) => self.succeed(tr("Gespeicherter Entwurf gelöscht.").into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.fail(tr("Es ist kein gespeicherter Entwurf vorhanden.").into());
            }
            Err(error) => self.fail(tr_fmt(
                "Entwurf konnte nicht gelöscht werden: {}",
                &[error.to_string()],
            )),
        }
    }

    fn reset(&mut self) {
        let kind = self.form.kind;
        let category = self.form.category;
        self.form = FeedbackForm {
            kind,
            category,
            ..FeedbackForm::default()
        };
        self.accept_findings = false;
        self.show_problems = false;
        self.succeed(
            tr("Formular zurückgesetzt. Ein gespeicherter Entwurf bleibt bestehen.").into(),
        );
    }

    fn show_notice(&self, ui: &mut egui::Ui) {
        if let Some((ok, message)) = &self.notice {
            let color = if *ok {
                ui.visuals().text_color()
            } else {
                ui.visuals().error_fg_color
            };
            ui.add_space(6.0);
            ui.colored_label(color, message);
        }
    }

    fn show_mail_buttons(&mut self, ui: &mut egui::Ui) {
        let available = CONTACT_EMAIL.is_some();
        ui.horizontal(|ui| {
            if ui
                .add_enabled(available, egui::Button::new(tr("Per E-Mail senden")))
                .on_disabled_hover_text(tr("Keine Kontaktadresse konfiguriert."))
                .clicked()
            {
                self.open_mail(true);
            }
            if ui
                .add_enabled(available, egui::Button::new(tr("E-Mail-Programm öffnen")))
                .on_disabled_hover_text(tr("Keine Kontaktadresse konfiguriert."))
                .clicked()
            {
                self.open_mail(false);
            }
        });
        ui.label(tr(
            "So senden Sie das PDF: 1. PDF speichern. 2. „Per E-Mail senden“ wählen. 3. Die gespeicherte Datei selbst an die E-Mail anhängen. 4. Den Versand im E-Mail-Programm bestätigen.",
        ));
    }

    fn show_privacy(ui: &mut egui::Ui) {
        ui.strong(tr("Datenschutz"));
        for line in [
            "NORTEX sendet nichts automatisch: keine Telemetrie, keine Fehlerberichte, keine Anhänge.",
            "Technische Angaben (Version, Betriebssystem, Architektur, Compiler) werden nur beigefügt, wenn Sie das ausdrücklich ankreuzen.",
            "Der Inhalt jeder Angabe ist vor dem Export sichtbar. Mögliche Passwörter, Tokens, E-Mail-Adressen und Benutzernamen in Pfaden werden erkannt und können geschwärzt werden.",
            "Ein Entwurf wird nur lokal gespeichert, wenn Sie „Entwurf lokal speichern“ wählen. Sie können ihn jederzeit löschen.",
            "Mit dem Versand der PDF-Datei übermitteln Sie die Angaben selbst an den Empfänger der E-Mail.",
        ] {
            ui.label(format!("• {}", tr(line)));
        }
    }

    fn show_help(&mut self, ui: &mut egui::Ui, manual_open: &mut bool, shortcuts_open: &mut bool) {
        ui.label(tr(
            "Handbuch, Tastenkürzel und Dokumentation finden Sie hier.",
        ));
        ui.horizontal(|ui| {
            if ui.button(tr("Handbuch im Fenster")).clicked() {
                *manual_open = true;
            }
            if ui.button(tr("Tastatur-Kürzel")).clicked() {
                *shortcuts_open = true;
            }
        });
        ui.add_space(6.0);
        ui.label(tr("Fehler, Wünsche oder Fehler in der Dokumentation können Sie über die Bereiche links melden."));
    }

    fn show_support(&mut self, ui: &mut egui::Ui, context: &egui::Context) {
        match CONTACT_EMAIL {
            Some(address) => {
                ui.horizontal(|ui| {
                    ui.label(format!("{}: {address}", tr("Kontakt")));
                    if ui.button(tr("Adresse kopieren")).clicked() {
                        context.copy_text(address.to_owned());
                        self.succeed(tr("Adresse in die Zwischenablage kopiert.").into());
                    }
                });
            }
            None => {
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    tr("Es ist keine Kontaktadresse konfiguriert. Der E-Mail-Versand ist deshalb deaktiviert; das PDF-Formular lässt sich trotzdem speichern."),
                );
            }
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button(tr("PDF-Formular herunterladen")).clicked() {
                self.export_pdf(false);
            }
        });
        ui.add_space(4.0);
        self.show_mail_buttons(ui);
        ui.add_space(10.0);
        Self::show_privacy(ui);
        self.show_notice(ui);
    }

    fn show_form(&mut self, ui: &mut egui::Ui) {
        let language = language();
        let form = &mut self.form;
        egui::Grid::new("feedback-general")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label(tr("Kategorie"));
                egui::ComboBox::from_id_salt("feedback-category")
                    .selected_text(tr(form.category.label()))
                    .show_ui(ui, |ui| {
                        for category in Category::ALL {
                            ui.selectable_value(&mut form.category, category, tr(category.label()));
                        }
                    });
                ui.end_row();
                ui.label(tr("Titel"));
                ui.add(egui::TextEdit::singleline(&mut form.title).desired_width(380.0));
                ui.end_row();
                ui.label(tr("Kontakt-E-Mail (optional)"));
                ui.add(egui::TextEdit::singleline(&mut form.contact_email).desired_width(380.0));
                ui.end_row();
            });
        text_area(ui, tr("Beschreibung"), &mut form.description, 5);
        if form.kind == Kind::Bug {
            text_area(ui, tr("Schritte zur Reproduktion"), &mut form.steps, 4);
            text_area(ui, tr("Erwartetes Verhalten"), &mut form.expected, 2);
            text_area(ui, tr("Tatsächliches Verhalten"), &mut form.actual, 2);
            text_area(ui, tr("Fehlermeldung"), &mut form.error_message, 3);
            ui.horizontal(|ui| {
                ui.label(tr("Häufigkeit des Auftretens"));
                egui::ComboBox::from_id_salt("feedback-frequency")
                    .selected_text(tr(form.frequency.label()))
                    .show_ui(ui, |ui| {
                        for frequency in Frequency::ALL {
                            ui.selectable_value(
                                &mut form.frequency,
                                frequency,
                                tr(frequency.label()),
                            );
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.label(tr("Betroffene Funktion oder Komponente"));
                ui.add(egui::TextEdit::singleline(&mut form.component).desired_width(260.0));
            });
        }

        ui.add_space(6.0);
        ui.checkbox(
            &mut form.include_technical,
            tr("Technische Informationen beifügen (nur mit meiner Zustimmung)"),
        );
        if form.include_technical {
            ui.horizontal(|ui| {
                ui.label(tr("Betroffene Extension"));
                ui.add(egui::TextEdit::singleline(&mut form.extension).desired_width(260.0));
            });
            text_area(
                ui,
                tr("Relevante Fehlermeldungen"),
                &mut form.log_excerpt,
                3,
            );
            ui.label(tr("Diese Angaben werden exportiert:"));
            let mut preview = form.technical_info(language).unwrap_or_default();
            ui.add(
                egui::TextEdit::multiline(&mut preview)
                    .desired_rows(4)
                    .desired_width(f32::INFINITY)
                    .interactive(false),
            );
        }

        let findings = self.form.scan_for_secrets();
        if !findings.is_empty() {
            ui.add_space(6.0);
            ui.colored_label(
                ui.visuals().warn_fg_color,
                tr("Mögliche vertrauliche Angaben gefunden:"),
            );
            for finding in &findings {
                ui.label(format!(
                    "• {}: {}",
                    tr(finding.field),
                    tr(finding.kind.label())
                ));
            }
            ui.horizontal(|ui| {
                if ui.button(tr("Automatisch schwärzen")).clicked() {
                    self.form.redact_all();
                }
                ui.checkbox(
                    &mut self.accept_findings,
                    tr("Trotzdem unverändert verwenden"),
                );
            });
        }

        if self.show_problems {
            for problem in self.form.validate() {
                ui.colored_label(ui.visuals().error_fg_color, tr(problem.message()));
            }
        }

        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            if ui.button(tr("Als PDF exportieren")).clicked() {
                self.export_pdf(true);
            }
            if ui.button(tr("Zurücksetzen")).clicked() {
                self.reset();
            }
            if ui.button(tr("Entwurf lokal speichern")).clicked() {
                self.save_draft();
            }
            if ui.button(tr("Entwurf laden")).clicked() {
                self.load_draft();
            }
            if ui.button(tr("Entwurf löschen")).clicked() {
                self.delete_draft();
            }
        });
        ui.label(tr_fmt(
            "Ein Entwurf wird nur auf Ihren Wunsch gespeichert: {}",
            &[feedback::draft_path().display().to_string()],
        ));
        ui.add_space(4.0);
        self.show_mail_buttons(ui);
        ui.add_space(8.0);
        ui.collapsing(tr("Datenschutzinformationen"), Self::show_privacy);
        self.show_notice(ui);
    }
}

fn text_area(ui: &mut egui::Ui, label: &str, value: &mut String, rows: usize) {
    ui.label(label);
    ui.add(
        egui::TextEdit::multiline(value)
            .desired_rows(rows)
            .desired_width(f32::INFINITY),
    );
}

impl NortexCodeApp {
    pub(crate) fn draw_feedback_window(&mut self, context: &egui::Context) {
        if !self.feedback.open {
            return;
        }
        let mut open = true;
        egui::Window::new(tr("Hilfe und Support"))
            .open(&mut open)
            .default_width(760.0)
            .default_height(620.0)
            .show(context, |ui| {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(190.0);
                        for section in SECTIONS {
                            let selected = self.feedback.section == section;
                            if ui.selectable_label(selected, tr(section.label())).clicked() {
                                self.feedback.open_section(section);
                            }
                        }
                    });
                    ui.separator();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.heading(tr(self.feedback.section.label()));
                            ui.add_space(6.0);
                            match self.feedback.section {
                                HelpSection::Help => self.feedback.show_help(
                                    ui,
                                    &mut self.manual_open,
                                    &mut self.shortcuts_open,
                                ),
                                HelpSection::Report(_) => self.feedback.show_form(ui),
                                HelpSection::Support => self.feedback.show_support(ui, context),
                            }
                        });
                    });
                });
            });
        self.feedback.open = open;
    }
}
