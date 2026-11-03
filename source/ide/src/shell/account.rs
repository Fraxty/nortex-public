//! Konto-Oberfläche: Profilmenü in der Titelleiste und Kontofenster (lokaler Modus).

use crate::*;
use nortex_ide::account::{AccountError, LOCAL_ONLY_DATA};
use nortex_ide::i18n::tr_fmt;

impl NortexCodeApp {
    pub(crate) fn draw_account_menu(&mut self, ui: &mut egui::Ui) {
        let status = self.account.status();
        ui.menu_button(tr("Konto"), |ui| {
            ui.label(egui::RichText::new(status.label()).strong());
            ui.label(
                egui::RichText::new(tr("Alle lokalen Funktionen laufen ohne Konto."))
                    .small()
                    .color(self.theme.palette().text_muted),
            );
            ui.separator();
            if ui.button(tr("Anmelden…")).clicked() {
                self.account_open = true;
                ui.close_menu();
            }
            if ui.button(tr("Konto und Synchronisierung…")).clicked() {
                self.account_open = true;
                ui.close_menu();
            }
            ui.separator();
            for item in ["KI-Funktionen", tr("Erweiterungen"), tr("Lizenz"), "Geräte"] {
                ui.add_enabled(
                    false,
                    egui::Button::new(tr_fmt("{} (geplant)", &[item.to_string()])),
                );
            }
            ui.separator();
            if ui
                .add_enabled(status.is_signed_in(), egui::Button::new(tr("Abmelden")))
                .clicked()
            {
                self.account.sign_out();
                ui.close_menu();
            }
        });
    }

    pub(crate) fn draw_account_window(&mut self, context: &egui::Context) {
        if !self.account_open {
            return;
        }
        let mut open = true;
        egui::Window::new(tr("Nortex-Konto"))
            .open(&mut open)
            .default_width(460.0)
            .show(context, |ui| {
                ui.heading(self.account.status().label());
                ui.label(
                    "Ein Konto ist optional. Editor, Compiler, Build und Projekte funktionieren \
                     vollständig lokal.",
                );
                ui.add_space(6.0);
                if ui.button(tr("Im Browser anmelden")).clicked() {
                    self.status = match self.account.sign_in() {
                        Ok(status) => status.label(),
                        Err(error @ AccountError::ServiceUnavailable) => error.to_string(),
                    };
                }
                ui.label(
                    egui::RichText::new(
                        "Der Kontodienst ist noch nicht verfügbar. Geplant: Anmeldung per OAuth2/OIDC mit PKCE \
                         über den Systembrowser; es werden keine Passwörter oder Secrets in der IDE gespeichert.",
                    )
                    .small()
                    .color(self.theme.palette().text_muted),
                );
                ui.separator();
                ui.strong(tr("Synchronisierung (wirkt erst nach Anmeldung)"));
                let mut changed = false;
                let sync = &mut self.settings.sync;
                changed |= ui.checkbox(&mut sync.settings, tr("Einstellungen")).changed();
                changed |= ui.checkbox(&mut sync.themes, tr("Themes (Farbschema, Dichte, Zoom)")).changed();
                changed |= ui.checkbox(&mut sync.shortcuts, tr("Tastenkürzel")).changed();
                changed |= ui.checkbox(&mut sync.extensions, tr("Liste der Erweiterungen")).changed();
                ui.label(tr(LOCAL_ONLY_DATA));
                if changed {
                    self.persist_settings();
                }
                ui.separator();
                ui.label(
                    egui::RichText::new(
                        tr("Weitere Phasen (geplant): Profil und Sitzung, Synchronisierung, KI-Dienste, Lizenzen und Geräte."),
                    )
                    .small()
                    .color(self.theme.palette().text_muted),
                );
            });
        self.account_open = open;
    }
}
