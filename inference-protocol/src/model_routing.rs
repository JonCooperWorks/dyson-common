//! Deterministic model-pattern routing for OpenAI-compatible gateways.
//!
//! The matcher owns no credentials or transport state. Applications attach
//! their own route payload and keep persistence, health checks, billing, and
//! authorization outside this crate.

use std::collections::HashMap;

/// A route selector and its application-defined payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRoute<T> {
    /// Exact model, trailing-star prefix, `*`, or a full regular expression.
    pub pattern: String,
    pub is_regex: bool,
    pub enabled: bool,
    pub value: T,
}

#[derive(Debug, Clone)]
struct RegexRoute<T> {
    pattern: String,
    regex: regex::Regex,
    value: T,
}

/// Compiled matcher with stable precedence: exact, longest prefix, regex in
/// lexical pattern order, then the universal `*` route.
#[derive(Debug, Clone)]
pub struct ModelRouter<T> {
    exact: HashMap<String, T>,
    prefixes: Vec<(String, String, T)>,
    regexes: Vec<RegexRoute<T>>,
    catch_all: Option<T>,
}

impl<T> Default for ModelRouter<T> {
    fn default() -> Self {
        Self {
            exact: HashMap::new(),
            prefixes: Vec::new(),
            regexes: Vec::new(),
            catch_all: None,
        }
    }
}

impl<T> ModelRouter<T> {
    /// Compile enabled routes. Invalid regex rules are returned to the caller
    /// and excluded, allowing hosts to reject them on write or report them at
    /// configuration load time.
    pub fn build(routes: impl IntoIterator<Item = ModelRoute<T>>) -> (Self, Vec<(String, String)>) {
        let mut router = Self::default();
        let mut errors = Vec::new();
        for route in routes {
            if !route.enabled {
                continue;
            }
            if route.is_regex {
                match regex::Regex::new(&route.pattern) {
                    Ok(regex) => router.regexes.push(RegexRoute {
                        pattern: route.pattern,
                        regex,
                        value: route.value,
                    }),
                    Err(error) => errors.push((route.pattern, error.to_string())),
                }
            } else if route.pattern == "*" {
                router.catch_all = Some(route.value);
            } else if let Some(prefix) = route.pattern.strip_suffix('*') {
                router
                    .prefixes
                    .push((prefix.to_owned(), route.pattern, route.value));
            } else {
                router.exact.insert(route.pattern, route.value);
            }
        }
        router
            .prefixes
            .sort_by(|(a, ap, _), (b, bp, _)| b.len().cmp(&a.len()).then_with(|| ap.cmp(bp)));
        router.regexes.sort_by(|a, b| a.pattern.cmp(&b.pattern));
        (router, errors)
    }

    pub fn resolve(&self, model: &str) -> Option<&T> {
        self.exact
            .get(model)
            .or_else(|| {
                self.prefixes
                    .iter()
                    .find(|(prefix, _, _)| model.starts_with(prefix))
                    .map(|(_, _, value)| value)
            })
            .or_else(|| {
                self.regexes
                    .iter()
                    .find(|route| route.regex.is_match(model))
                    .map(|route| &route.value)
            })
            .or(self.catch_all.as_ref())
    }

    pub fn catch_all(&self) -> Option<&T> {
        self.catch_all.as_ref()
    }

    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.exact
            .values()
            .chain(self.prefixes.iter().map(|(_, _, value)| value))
            .chain(self.regexes.iter().map(|route| &route.value))
            .chain(self.catch_all.iter())
    }

    pub fn len(&self) -> usize {
        self.exact.len()
            + self.prefixes.len()
            + self.regexes.len()
            + usize::from(self.catch_all.is_some())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Expand a regex capture template such as `upstream/$1` or `${family}`.
pub fn expand_model_rewrite(pattern: &str, model: &str, template: &str) -> Option<String> {
    let regex = regex::Regex::new(pattern).ok()?;
    let captures = regex.captures(model)?;
    let mut output = String::new();
    captures.expand(template, &mut output);
    Some(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_in_specificity_order() {
        let (router, errors) = ModelRouter::build([
            ModelRoute {
                pattern: "*".into(),
                is_regex: false,
                enabled: true,
                value: "all",
            },
            ModelRoute {
                pattern: "openai/*".into(),
                is_regex: false,
                enabled: true,
                value: "provider",
            },
            ModelRoute {
                pattern: "openai/gpt-5".into(),
                is_regex: false,
                enabled: true,
                value: "exact",
            },
            ModelRoute {
                pattern: r"^anthropic/claude-.*$".into(),
                is_regex: true,
                enabled: true,
                value: "regex",
            },
        ]);
        assert!(errors.is_empty());
        assert_eq!(router.resolve("openai/gpt-5"), Some(&"exact"));
        assert_eq!(router.resolve("openai/gpt-4"), Some(&"provider"));
        assert_eq!(router.resolve("anthropic/claude-opus"), Some(&"regex"));
        assert_eq!(router.resolve("mistral/large"), Some(&"all"));
    }

    #[test]
    fn expands_named_and_numbered_regex_captures() {
        assert_eq!(
            expand_model_rewrite(
                r"^vendor/(?P<family>[^/]+)/(.+)$",
                "vendor/claude/opus",
                "mirror/${family}/$2"
            ),
            Some("mirror/claude/opus".into())
        );
    }
}
