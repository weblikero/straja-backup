//! Presentation-only localization. Persistent plans, run outcomes and paths remain unchanged.
use serde::Deserialize;
use std::{
    path::Path,
    sync::{
        OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(usize)]
pub enum Language {
    #[default]
    English,
    German,
    Spanish,
    Portuguese,
    French,
    Italian,
    Romanian,
}
impl Language {
    pub const ALL: [Self; 7] = [
        Self::English,
        Self::German,
        Self::Spanish,
        Self::Portuguese,
        Self::French,
        Self::Italian,
        Self::Romanian,
    ];
    pub fn code(self) -> &'static str {
        ["en", "de", "es", "pt", "fr", "it", "ro"][self as usize]
    }
    pub fn from_code(code: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|l| l.code() == code.trim())
            .unwrap_or_default()
    }
}
impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            [
                "English",
                "Deutsch",
                "Español",
                "Português",
                "Français",
                "Italiano",
                "Română",
            ][*self as usize],
        )
    }
}
static LANGUAGE: AtomicUsize = AtomicUsize::new(0);
pub fn current() -> Language {
    Language::ALL[LANGUAGE.load(Ordering::Relaxed)]
}
pub fn set(language: Language) {
    LANGUAGE.store(language as usize, Ordering::Relaxed);
}
pub fn read_preference(path: &Path) -> Language {
    std::fs::read_to_string(path)
        .ok()
        .filter(|s| s.len() < 32)
        .map(|s| Language::from_code(&s))
        .unwrap_or_default()
}
pub fn write_preference(path: &Path, language: Language) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Only this small, dedicated UI preference is changed; the database is untouched.
    std::fs::write(path, language.code())
}
pub fn reload() {
    if let Ok(dir) = crate::security::data_dir() {
        set(read_preference(&dir.join("language.txt")));
    }
}
pub fn save(language: Language) -> anyhow::Result<()> {
    write_preference(&crate::security::data_dir()?.join("language.txt"), language)?;
    set(language);
    Ok(())
}
#[derive(Deserialize)]
struct Entry {
    source: String,
    parts: Vec<String>,
    values: Vec<String>,
}
fn catalog() -> &'static Vec<Entry> {
    static CATALOG: OnceLock<Vec<Entry>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let mut rows: Vec<Entry> = serde_json::from_str(include_str!("../locales/catalog.json"))
            .expect("validated locale catalog");
        // Specific templates win over short progress prefixes.
        rows.sort_by_key(|e| std::cmp::Reverse(e.parts.iter().map(String::len).sum::<usize>()));
        rows
    })
}
fn captures<'a>(parts: &[String], input: &'a str) -> Option<Vec<&'a str>> {
    let mut tail = input.strip_prefix(&parts[0])?;
    let mut values = Vec::new();
    for (index, delimiter) in parts.iter().enumerate().skip(1) {
        if index == parts.len() - 1 {
            values.push(tail.strip_suffix(delimiter)?);
            return Some(values);
        }
        if delimiter.is_empty() {
            return None;
        }
        let (value, rest) = tail.split_once(delimiter)?;
        values.push(value);
        tail = rest;
    }
    None
}
fn render(template: &str, values: &[String]) -> String {
    // Single pass: braces in user names/paths are never interpreted as placeholders.
    let mut result = String::new();
    let mut rest = template;
    while let Some((before, after)) = rest.split_once('{') {
        result.push_str(before);
        if let Some((index, tail)) = after.split_once('}')
            && let Ok(index) = index.parse::<usize>()
            && let Some(value) = values.get(index)
        {
            result.push_str(value);
            rest = tail;
        } else {
            result.push('{');
            rest = after;
        }
    }
    result.push_str(rest);
    result
}
fn nested(source: &str, index: usize) -> bool {
    // Only application-owned status/message slots. Never translate user data.
    match source {
        "{} în curs"
        | "{} · {}: cerere acceptată, se pregătește operația…"
        | "Săptămânal · {} · {hour:02}:{minute:02}"
        | "Straja · {} în curs · {:.0}%"
        | "Straja · {} activ · {} planuri" => index == 0,
        "ULTIMA ÎNCERCARE · {} · {}"
        | "PASUL {} DIN 4  ·  {}"
        | "Straja · {}: {} · Deschide aplicația pentru detalii" => index == 1,
        "{} · {} · Backupul se pornește din pagina planului" => true,
        "Program: {}"
        | "MOD: {}"
        | "Mod: {}"
        | "Straja · {error}"
        | " ATENȚIE: backupul nou este salvat, dar curățarea copiilor vechi nu s-a încheiat: {e:#}"
        | "Operație întreruptă: {e}"
        | "Programare: {e}" => true,
        " Retenție: {count} copii vechi eliminate; {}." => index == 1,
        _ => false,
    }
}
pub fn tr(input: impl AsRef<str>) -> String {
    translate(current(), input.as_ref())
}
pub fn translate(language: Language, input: &str) -> String {
    translate_depth(language, input, 0)
}
fn translate_depth(language: Language, input: &str, depth: usize) -> String {
    if language == Language::Romanian || depth > 8 {
        return input.to_owned();
    }
    // The engine appends these clauses only to a successful backup detail.
    // Their colons cannot occur in a normal Windows file/directory name.
    if input.starts_with("Backup finalizat.")
        || input.starts_with("Backup necriptat finalizat")
        || input.starts_with("Arhivă finalizată")
    {
        let boundary = [
            " Retenție: ",
            " ATENȚIE: ",
            " Eroare la salvarea programării: ",
        ]
        .iter()
        .filter_map(|marker| input.find(marker))
        .min();
        if let Some(boundary) = boundary {
            return format!(
                "{}{}",
                translate_depth(language, &input[..boundary], depth + 1),
                translate_depth(language, &input[boundary..], depth + 1)
            );
        }
    }
    for entry in catalog() {
        if entry.parts.len() == 1 && entry.source == input {
            return entry.values[language as usize].clone();
        }
    }
    for entry in catalog().iter().filter(|e| e.parts.len() > 1) {
        if let Some(args) = captures(&entry.parts, input) {
            let args: Vec<String> = args
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    if nested(&entry.source, i) {
                        translate_depth(language, s, depth + 1)
                    } else {
                        (*s).to_owned()
                    }
                })
                .collect();
            return render(&entry.values[language as usize], &args);
        }
    }
    // Legacy anyhow context chains. A drive colon is followed by a slash, not a space.
    if let Some((head, tail)) = input.split_once(": ") {
        return format!(
            "{}: {}",
            translate_depth(language, head, depth + 1),
            translate_depth(language, tail, depth + 1)
        );
    }
    input.to_owned() // Native OS/Restic diagnostics remain verbatim.
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_and_saved_language_roundtrip_without_touching_other_files() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("settings/language.txt");
        assert_eq!(Language::default(), Language::English);
        assert_eq!(read_preference(&path), Language::English);
        let canary = temp.path().join("plans.sqlite");
        std::fs::write(&canary, b"existing plans").unwrap();
        for language in Language::ALL {
            write_preference(&path, language).unwrap();
            assert_eq!(read_preference(&path), language);
        }
        std::fs::write(&path, "unknown\n").unwrap();
        assert_eq!(read_preference(&path), Language::English);
        assert_eq!(std::fs::read(&canary).unwrap(), b"existing plans");
        assert!(write_preference(temp.path(), Language::French).is_err());
    }
    #[test]
    fn every_message_renders_in_every_language_with_all_arguments() {
        let mut seen = std::collections::HashSet::new();
        for entry in catalog() {
            assert!(seen.insert(&entry.source));
            assert_eq!(entry.values.len(), 6, "{}", entry.source);
            assert!(entry.values.iter().all(|s| !s.is_empty()));
            let args: Vec<String> = (0..entry.parts.len() - 1)
                .map(|i| format!("VALUE_{i}"))
                .collect();
            let mut original = entry.parts[0].clone();
            for (value, tail) in args.iter().zip(entry.parts.iter().skip(1)) {
                original.push_str(value);
                original.push_str(tail);
            }
            for language in Language::ALL {
                let expected = if language == Language::Romanian {
                    original.clone()
                } else {
                    render(&entry.values[language as usize], &args)
                };
                assert_eq!(
                    translate(language, &original),
                    expected,
                    "{} / {}",
                    language,
                    entry.source
                );
            }
        }
    }
    #[test]
    fn retention_and_cleanup_warning_translate_with_success_detail() {
        let detail = r"Backup necriptat finalizat și verificat. Fișierele pot fi deschise și copiate din E:\Backups\copy Retenție: 2 copii vechi eliminate; Păstrează ultimele 7 copii (inclusiv cea mai nouă).";
        let value = translate(Language::English, detail);
        assert_eq!(
            value,
            r"Unencrypted backup completed and verified. Open and copy files from E:\Backups\copy Retention: 2 old backups removed; Keep the last 7 backups (including the newest)."
        );
        let warning = "Backup finalizat. Copia este disponibilă în Restaurare. ATENȚIE: backupul nou este salvat, dar curățarea copiilor vechi nu s-a încheiat: Curățare oprită: copie incompletă.";
        let value = translate(Language::English, warning);
        assert!(
            value.contains("WARNING") && value.contains("Cleanup stopped"),
            "{value}"
        );
        assert!(!value.contains("ATENȚIE") && !value.contains("Curățare"));
    }

    #[test]
    fn legacy_junction_error_translates_without_touching_paths_or_names() {
        let source = r"C:\laragon\www\example-suite\public\build";
        let destination = r"\\?\E:\bkp_laragon\.in-progress-Backup-20261018-test";
        let detail = format!(
            "Backup nefinalizat. Copia parțială se află în {destination}; versiunile anterioare sunt intacte.: Legătură/junction/reparse point neacceptat: {source}. Alege folderul real sau exclude legătura."
        );
        for language in Language::ALL
            .into_iter()
            .filter(|l| *l != Language::Romanian)
        {
            let translated = translate(language, &detail);
            assert!(
                translated.contains(source) && translated.contains(destination),
                "{language}: {translated}"
            );
            assert!(
                !translated.contains("nefinalizat") && !translated.contains("neacceptat"),
                "{language}: {translated}"
            );
            for name in ["Reușit", "Error {1}", "A · B: C", "写真 și Grüße"] {
                let translated =
                    translate(language, &format!("Planul «{name}» a fost actualizat."));
                assert!(
                    translated.contains(name),
                    "User names are not translation keys: {translated}"
                );
            }
        }
        assert_eq!(
            translate(Language::English, "Săptămânal · Luni · 19:00"),
            "Weekly · Monday · 19:00"
        );
        assert_eq!(
            translate(Language::English, "ULTIMA ÎNCERCARE · 2026-10-19 · Reușit"),
            "LAST ATTEMPT · 2026-10-19 · Successful"
        );
        assert_eq!(
            translate(Language::German, "Reguli: Reușit"),
            "Regeln: Reușit"
        );
        assert_eq!(
            translate(Language::English, "native error 0x1234"),
            "native error 0x1234"
        );
    }
}
