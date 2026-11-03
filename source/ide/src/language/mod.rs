//! Multi-language file recognition for NORTEX Code.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageId {
    Nortex,
    NortexProject,
    NortexPackage,
    Rust,
    CSharp,
    Cpp,
    C,
    Python,
    JavaScript,
    TypeScript,
    Html,
    Css,
    Json,
    Toml,
    Markdown,
    PlainText,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LanguageConfiguration {
    pub id: LanguageId,
    pub name: &'static str,
    pub extensions: &'static [&'static str],
}

pub const SUPPORTED_LANGUAGES: &[LanguageConfiguration] = &[
    LanguageConfiguration {
        id: LanguageId::Nortex,
        name: "NORTEX",
        extensions: &["nx"],
    },
    LanguageConfiguration {
        id: LanguageId::NortexProject,
        name: "NORTEX Project",
        extensions: &["nortex"],
    },
    LanguageConfiguration {
        id: LanguageId::NortexPackage,
        name: "NORTEX Package",
        extensions: &["ntx"],
    },
    LanguageConfiguration {
        id: LanguageId::Rust,
        name: "Rust",
        extensions: &["rs"],
    },
    LanguageConfiguration {
        id: LanguageId::CSharp,
        name: "C#",
        extensions: &["cs"],
    },
    LanguageConfiguration {
        id: LanguageId::Cpp,
        name: "C++",
        extensions: &["cpp", "cc", "cxx", "hpp", "hh", "hxx", "h"],
    },
    LanguageConfiguration {
        id: LanguageId::C,
        name: "C",
        extensions: &["c"],
    },
    LanguageConfiguration {
        id: LanguageId::Python,
        name: "Python",
        extensions: &["py"],
    },
    LanguageConfiguration {
        id: LanguageId::JavaScript,
        name: "JavaScript",
        extensions: &["js", "jsx"],
    },
    LanguageConfiguration {
        id: LanguageId::TypeScript,
        name: "TypeScript",
        extensions: &["ts", "tsx"],
    },
    LanguageConfiguration {
        id: LanguageId::Html,
        name: "HTML",
        extensions: &["html", "htm"],
    },
    LanguageConfiguration {
        id: LanguageId::Css,
        name: "CSS",
        extensions: &["css"],
    },
    LanguageConfiguration {
        id: LanguageId::Json,
        name: "JSON",
        extensions: &["json"],
    },
    LanguageConfiguration {
        id: LanguageId::Toml,
        name: "TOML",
        extensions: &["toml"],
    },
    LanguageConfiguration {
        id: LanguageId::Markdown,
        name: "Markdown",
        extensions: &["md", "markdown"],
    },
];

/// Accepts an extension with or without a leading dot.
pub fn detect_by_extension(extension: &str) -> LanguageId {
    let extension = extension.trim_start_matches('.');
    SUPPORTED_LANGUAGES
        .iter()
        .find(|language| {
            language
                .extensions
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(extension))
        })
        .map(|language| language.id)
        .unwrap_or(LanguageId::PlainText)
}

pub const NORTEX: LanguageConfiguration = LanguageConfiguration {
    id: LanguageId::Nortex,
    name: "NORTEX",
    extensions: &["nx"],
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_nortex_project_and_package_formats() {
        assert_eq!(detect_by_extension("nortex"), LanguageId::NortexProject);
        assert_eq!(detect_by_extension(".ntx"), LanguageId::NortexPackage);
        assert_eq!(detect_by_extension("nx"), LanguageId::Nortex);
    }

    #[test]
    fn recognizes_documented_cpp_js_ts_and_html_extensions() {
        for extension in ["h", "hpp", "cc", "cxx", "cpp"] {
            assert_eq!(detect_by_extension(extension), LanguageId::Cpp);
        }
        assert_eq!(detect_by_extension("tsx"), LanguageId::TypeScript);
        assert_eq!(detect_by_extension("jsx"), LanguageId::JavaScript);
        assert_eq!(detect_by_extension("htm"), LanguageId::Html);
    }
}
