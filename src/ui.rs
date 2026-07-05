//! Shared UI theme primitives for server-rendered pages.
//!
//! The React apps consume the same files through the `dyson-common-ui` npm
//! package. Rust-rendered pages cannot safely depend on `node_modules`, so this
//! module exposes the same token CSS and cookie/theme contract from the Rust
//! crate.

pub const THEME_COOKIE: &str = "dyson-theme";
pub const DESIGN_TOKENS_CSS: &str = include_str!("../ui/tokens.css");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    System,
    Light,
    Dark,
}

impl ThemeMode {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "system" => Some(Self::System),
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            _ => None,
        }
    }

    pub const fn html_attr(self) -> &'static str {
        match self {
            Self::System => "",
            Self::Light => r#" data-theme="light""#,
            Self::Dark => r#" data-theme="dark""#,
        }
    }

    pub const fn theme_color_meta(self) -> &'static str {
        match self {
            Self::System => {
                r##"<meta name="theme-color" content="#161922" media="(prefers-color-scheme: dark)">
<meta name="theme-color" content="#ffffff" media="(prefers-color-scheme: light)">"##
            }
            Self::Light => r##"<meta name="theme-color" content="#ffffff">"##,
            Self::Dark => r##"<meta name="theme-color" content="#161922">"##,
        }
    }
}

pub fn first_paint_theme_script(storage_key: &str) -> String {
    format!(
        r#"<script>
try {{
  var c = (document.cookie.match(/(?:^|;\s*){cookie}=([^;]*)/) || [])[1];
  var m = c ? decodeURIComponent(c) : localStorage.getItem('{storage_key}');
  if (m === 'light' || m === 'dark') document.documentElement.setAttribute('data-theme', m);
  else if (m === 'system') document.documentElement.removeAttribute('data-theme');
}} catch (e) {{}}
</script>"#,
        cookie = THEME_COOKIE,
        storage_key = storage_key,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_modes_match_shared_cookie_contract() {
        assert_eq!(ThemeMode::parse("system"), Some(ThemeMode::System));
        assert_eq!(ThemeMode::parse("light"), Some(ThemeMode::Light));
        assert_eq!(ThemeMode::parse("dark"), Some(ThemeMode::Dark));
        assert_eq!(ThemeMode::parse("nope"), None);

        assert_eq!(ThemeMode::System.html_attr(), "");
        assert_eq!(ThemeMode::Light.html_attr(), r#" data-theme="light""#);
        assert_eq!(ThemeMode::Dark.html_attr(), r#" data-theme="dark""#);
    }

    #[test]
    fn design_tokens_are_the_blue_dyson_palette() {
        assert!(DESIGN_TOKENS_CSS.contains("--accent: #6ea9ff"));
        assert!(DESIGN_TOKENS_CSS.contains("--accent: #2563eb"));
        assert!(DESIGN_TOKENS_CSS.contains(":root[data-theme=\"light\"]"));
        assert!(DESIGN_TOKENS_CSS.contains(":root:not([data-theme])"));
    }

    #[test]
    fn first_paint_script_uses_cookie_then_storage_key() {
        let script = first_paint_theme_script("swarm-theme");
        assert!(script.contains("dyson-theme"));
        assert!(script.contains("localStorage.getItem('swarm-theme')"));
        assert!(script.contains("data-theme"));
    }
}
