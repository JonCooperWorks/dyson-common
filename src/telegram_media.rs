//! Pure Telegram/media attachment policy shared by Dyson runtimes.
//!
//! Downloading and model invocation remain transport-specific. This module is
//! the single source of truth for accepted document types, effective MIME
//! resolution, and bounded-download defaults.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct DownloadLimits {
    pub image_max_bytes: u64,
    pub audio_max_bytes: u64,
    pub document_max_bytes: u64,
    pub text_max_bytes: u64,
}

impl Default for DownloadLimits {
    fn default() -> Self {
        Self {
            image_max_bytes: 50 * 1024 * 1024,
            audio_max_bytes: 50 * 1024 * 1024,
            document_max_bytes: 200 * 1024 * 1024,
            text_max_bytes: 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentKind {
    Image,
    Pdf,
    Office,
    Text,
    Binary,
}

impl DownloadLimits {
    #[must_use]
    pub fn for_document(self, kind: DocumentKind) -> u64 {
        match kind {
            DocumentKind::Image => self.image_max_bytes,
            DocumentKind::Text => self.text_max_bytes,
            DocumentKind::Pdf | DocumentKind::Office | DocumentKind::Binary => {
                self.document_max_bytes
            }
        }
    }
}

#[must_use]
pub fn classify_document(mime: &str, file_name: Option<&str>) -> DocumentKind {
    if mime.starts_with("image/") {
        return DocumentKind::Image;
    }
    if mime == "application/pdf" {
        return DocumentKind::Pdf;
    }
    if is_office_mime(mime) {
        return DocumentKind::Office;
    }
    if is_text_like_mime(mime) {
        return DocumentKind::Text;
    }
    if matches!(mime, "" | "application/octet-stream")
        && let Some(ext) = file_name.and_then(extension_of)
    {
        if is_office_extension(&ext) {
            return DocumentKind::Office;
        }
        if is_text_extension(&ext) {
            return DocumentKind::Text;
        }
        if ext == "pdf" {
            return DocumentKind::Pdf;
        }
        if is_image_extension(&ext) {
            return DocumentKind::Image;
        }
    }
    DocumentKind::Binary
}

/// Resolve Telegram's missing/generic MIME using the accepted file extension.
#[must_use]
pub fn effective_mime(original: &str, kind: DocumentKind, file_name: Option<&str>) -> String {
    match kind {
        DocumentKind::Image if !original.starts_with("image/") => file_name
            .and_then(extension_of)
            .and_then(|ext| image_mime_for_extension(&ext))
            .unwrap_or("application/octet-stream")
            .to_owned(),
        DocumentKind::Pdf if original != "application/pdf" => "application/pdf".to_owned(),
        DocumentKind::Office if !is_office_mime(original) => file_name
            .and_then(extension_of)
            .and_then(|ext| office_mime_for_extension(&ext))
            .unwrap_or(original)
            .to_owned(),
        DocumentKind::Text if !is_text_like_mime(original) => "text/plain".to_owned(),
        _ => original.to_owned(),
    }
}

#[must_use]
pub fn extension_of(name: &str) -> Option<String> {
    let dot = name.rfind('.')?;
    let ext = &name[dot + 1..];
    (!ext.is_empty()).then(|| ext.to_ascii_lowercase())
}

#[must_use]
pub fn is_text_like_mime(mime: &str) -> bool {
    mime.starts_with("text/")
        || matches!(
            mime,
            "application/json"
                | "application/xml"
                | "application/javascript"
                | "application/x-yaml"
                | "application/yaml"
                | "application/toml"
                | "application/x-sh"
                | "application/x-shellscript"
        )
}

#[must_use]
pub fn is_office_mime(mime: &str) -> bool {
    matches!(
        mime,
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            | "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
            | "application/vnd.openxmlformats-officedocument.presentationml.presentation"
            | "application/msword"
            | "application/vnd.ms-excel"
            | "application/vnd.ms-powerpoint"
    )
}

#[must_use]
pub fn is_office_extension(ext: &str) -> bool {
    matches!(ext, "docx" | "xlsx" | "pptx" | "doc" | "xls" | "ppt")
}

#[must_use]
pub fn is_image_extension(ext: &str) -> bool {
    matches!(
        ext,
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "tif" | "tiff"
    )
}

#[must_use]
pub fn is_text_extension(ext: &str) -> bool {
    matches!(
        ext,
        "md" | "markdown"
            | "txt"
            | "rst"
            | "log"
            | "csv"
            | "tsv"
            | "json"
            | "jsonl"
            | "ndjson"
            | "yaml"
            | "yml"
            | "toml"
            | "ini"
            | "cfg"
            | "conf"
            | "env"
            | "rs"
            | "go"
            | "py"
            | "pyi"
            | "js"
            | "mjs"
            | "cjs"
            | "ts"
            | "tsx"
            | "jsx"
            | "rb"
            | "sh"
            | "bash"
            | "zsh"
            | "fish"
            | "c"
            | "h"
            | "cpp"
            | "hpp"
            | "cc"
            | "hh"
            | "cxx"
            | "hxx"
            | "java"
            | "kt"
            | "kts"
            | "swift"
            | "m"
            | "mm"
            | "php"
            | "pl"
            | "lua"
            | "sql"
            | "html"
            | "htm"
            | "css"
            | "scss"
            | "sass"
            | "less"
            | "xml"
            | "svg"
            | "dockerfile"
            | "makefile"
            | "mk"
            | "lock"
            | "sum"
            | "mod"
            | "gitignore"
            | "gitattributes"
            | "editorconfig"
            | "r"
            | "scala"
            | "clj"
            | "ex"
            | "exs"
            | "erl"
            | "hs"
            | "elm"
            | "dart"
            | "vue"
            | "svelte"
            | "tf"
            | "hcl"
            | "proto"
            | "graphql"
            | "gql"
    )
}

fn office_mime_for_extension(ext: &str) -> Option<&'static str> {
    match ext {
        "docx" => Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
        "xlsx" => Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
        "pptx" => Some("application/vnd.openxmlformats-officedocument.presentationml.presentation"),
        "doc" => Some("application/msword"),
        "xls" => Some("application/vnd.ms-excel"),
        "ppt" => Some("application/vnd.ms-powerpoint"),
        _ => None,
    }
}

fn image_mime_for_extension(ext: &str) -> Option<&'static str> {
    match ext {
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        "tif" | "tiff" => Some("image/tiff"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_mime_uses_supported_extensions() {
        assert_eq!(
            classify_document("application/octet-stream", Some("plan.pdf")),
            DocumentKind::Pdf
        );
        assert_eq!(
            classify_document("", Some("photo.PNG")),
            DocumentKind::Image
        );
        assert_eq!(
            classify_document("application/octet-stream", Some("lease.docx")),
            DocumentKind::Office
        );
        assert_eq!(
            classify_document("application/octet-stream", Some("notes.md")),
            DocumentKind::Text
        );
        assert_eq!(
            classify_document("application/octet-stream", Some("archive.zip")),
            DocumentKind::Binary
        );
    }

    #[test]
    fn effective_mime_follows_extension_fallback() {
        assert_eq!(
            effective_mime("application/octet-stream", DocumentKind::Pdf, Some("x.pdf")),
            "application/pdf"
        );
        assert_eq!(
            effective_mime("", DocumentKind::Image, Some("x.png")),
            "image/png"
        );
        assert_eq!(
            effective_mime("", DocumentKind::Text, Some("x.rs")),
            "text/plain"
        );
    }
}
