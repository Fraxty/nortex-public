//! Schreibt das leere PDF-Formular nach `docs/feedback/` (de und en).
//! Aufruf: `cargo run -p nortex-ide --example write_feedback_form`

use nortex_ide::feedback::{CONTACT_EMAIL, pdf};
use nortex_ide::i18n::Language;

fn main() -> std::io::Result<()> {
    for (language, folder) in [(Language::German, "de"), (Language::English, "en")] {
        let directory = std::path::Path::new("docs").join("feedback").join(folder);
        std::fs::create_dir_all(&directory)?;
        let bytes = pdf::build_form(language, CONTACT_EMAIL, &[]);
        std::fs::write(directory.join(pdf::FILE_NAME), bytes)?;
    }
    Ok(())
}
