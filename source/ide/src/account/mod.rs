//! Nortex-Konto: lokaler Modus, Statusmodell und Schnittstelle zu künftigen Online-Diensten.
//!
//! Der lokale Kern der IDE funktioniert ohne Konto. Es existiert noch kein Backend:
//! [`LocalOnlyService`] meldet daher ehrlich [`AccountError::ServiceUnavailable`], statt
//! eine Anmeldung vorzutäuschen. Ein späterer Dienst (OAuth2/OIDC mit PKCE über den
//! Systembrowser, Tokens nur geschützt im Betriebssystem-Speicher, keine Secrets im Client)
//! implementiert [`AccountService`], ohne dass die Oberfläche geändert werden muss.

use crate::i18n::{tr, tr_fmt};
use serde::{Deserialize, Serialize};

/// Angemeldetes Profil. Enthält absichtlich keine Tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub display_name: String,
    pub email: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountStatus {
    /// Kein Konto, volle lokale Funktionalität.
    Local,
    SignedIn(Profile),
    /// Profil bekannt, Dienst aber nicht erreichbar.
    Offline(Profile),
}

impl AccountStatus {
    pub fn label(&self) -> String {
        match self {
            Self::Local => tr("Lokaler Modus").into(),
            Self::SignedIn(profile) => {
                tr_fmt("Angemeldet: {}", &[profile.display_name.to_string()])
            }
            Self::Offline(profile) => tr_fmt("Offline: {}", &[profile.display_name.to_string()]),
        }
    }

    pub fn is_signed_in(&self) -> bool {
        !matches!(self, Self::Local)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountError {
    /// Es ist kein Online-Dienst eingerichtet.
    ServiceUnavailable,
}

impl std::fmt::Display for AccountError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ServiceUnavailable => formatter.write_str(
                tr("Der Nortex-Kontodienst ist noch nicht verfügbar. Alle lokalen Funktionen bleiben nutzbar."),
            ),
        }
    }
}

impl std::error::Error for AccountError {}

/// Datenklassen, die später synchronisiert werden dürfen. Quellcode gehört nicht dazu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SyncPreferences {
    pub settings: bool,
    pub themes: bool,
    pub shortcuts: bool,
    pub extensions: bool,
}

impl SyncPreferences {
    pub fn any(&self) -> bool {
        self.settings || self.themes || self.shortcuts || self.extensions
    }
}

pub const LOCAL_ONLY_DATA: &str = "Quellcode, Projekte und Dateien verlassen den Rechner nie.";

/// Schnittstelle zum Kontodienst (Anmeldung, Sitzung, Abmeldung).
pub trait AccountService {
    fn status(&self) -> AccountStatus;
    fn sign_in(&mut self) -> Result<AccountStatus, AccountError>;
    fn sign_out(&mut self);
}

/// Standarddienst ohne Backend: bleibt dauerhaft im lokalen Modus.
#[derive(Debug, Default)]
pub struct LocalOnlyService;

impl AccountService for LocalOnlyService {
    fn status(&self) -> AccountStatus {
        AccountStatus::Local
    }

    fn sign_in(&mut self) -> Result<AccountStatus, AccountError> {
        Err(AccountError::ServiceUnavailable)
    }

    fn sign_out(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_service_never_pretends_to_sign_in() {
        let mut service = LocalOnlyService;
        assert_eq!(service.status(), AccountStatus::Local);
        assert_eq!(service.sign_in(), Err(AccountError::ServiceUnavailable));
        service.sign_out();
        assert!(!service.status().is_signed_in());
        assert_eq!(service.status().label(), "Lokaler Modus");
    }

    #[test]
    fn sync_is_off_by_default() {
        assert!(!SyncPreferences::default().any());
    }

    #[test]
    fn status_labels_name_the_profile() {
        let profile = Profile {
            display_name: "Ada".into(),
            email: "ada@example.org".into(),
        };
        assert_eq!(
            AccountStatus::SignedIn(profile.clone()).label(),
            "Angemeldet: Ada"
        );
        assert!(AccountStatus::Offline(profile).is_signed_in());
    }
}
