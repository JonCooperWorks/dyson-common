//! Other secret detectors, run beside Kingfisher as the programs their
//! authors ship.
//!
//! No one detector finds every secret: each has formats the others lack
//! and its own idea of what a generic key or password looks like. These
//! three are run over the same texts:
//!
//! * Betterleaks (`betterleaks dir`), its whole default configuration;
//! * TruffleHog (`trufflehog filesystem --no-verification`), less the
//!   detectors in `rules/trufflehog-exclude.txt` (see [`External::exclude`]);
//! * Nosey Parker (`noseyparker scan`, then `report`), its default ruleset.
//!
//! None of them is ever let present a key to its issuer: TruffleHog runs
//! with verification off, Betterleaks' validation is opt-in and never asked
//! for, and Nosey Parker has none. Each runs with an empty environment and
//! a scratch home, so it sees none of the host's credentials.
//!
//! They read files, so a batch of texts is written to a scratch directory,
//! read by each program once, side by side, and removed. Each takes seconds
//! to start, so give [`External::run`] many texts at a time, never one, and
//! remember what it found by the text's hash if the same texts come back.
//! Pass what it returns for a text to
//! [`Scanner::scan_at`](crate::Scanner::scan_at) to merge it with
//! Kingfisher's findings.
//!
//! A program that is missing, fails or prints what cannot be read is an
//! error: a detector that could not run is never read as one that found
//! nothing.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

use base64::Engine;
use serde_json::Value;

use crate::Error;
pub use crate::hit::Extra;

/// Texts shorter than this are not read, and values shorter are not kept.
pub const SHORTEST: usize = 8;
/// TruffleHog detectors left out by default, one a line (see the file).
const TRUFFLEHOG_EXCLUDE: &str = include_str!("../rules/trufflehog-exclude.txt");

static SCRATCH: AtomicU64 = AtomicU64::new(0);

type Skip = Box<dyn Fn(&str) -> bool + Send + Sync>;

/// The three programs, in one directory.
pub struct External {
    dir: PathBuf,
    exclude: Vec<String>,
    skip: Option<Skip>,
}

impl std::fmt::Debug for External {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("External")
            .field("dir", &self.dir)
            .field("exclude", &self.exclude)
            .finish_non_exhaustive()
    }
}

fn err(context: impl std::fmt::Display) -> impl FnOnce(std::io::Error) -> Error {
    move |e| Error(format!("{context}: {e}"))
}

impl External {
    /// The directory holding `betterleaks`, `trufflehog` and `noseyparker`.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            exclude: TRUFFLEHOG_EXCLUDE
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .map(str::to_owned)
                .collect(),
            skip: None,
        }
    }

    /// The TruffleHog detectors left out, by the name TruffleHog gives
    /// them, in place of the defaults.
    pub fn exclude(mut self, detectors: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.exclude = detectors.into_iter().map(Into::into).collect();
        self
    }

    /// Values Nosey Parker's capture groups hold that are not the secret:
    /// a host, address or mailbox beside a password in a connection string.
    /// A value for which `skip` is true is dropped.
    pub fn skip_value(mut self, skip: impl Fn(&str) -> bool + Send + Sync + 'static) -> Self {
        self.skip = Some(Box::new(skip));
        self
    }

    fn program(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    /// A detector's command: nothing of this process's environment (its
    /// database URL, its keys), a home of its own to write in.
    fn command(&self, name: &str, home: &Path) -> Command {
        let mut command = Command::new(self.program(name));
        command
            .env_clear()
            .env("HOME", home)
            .env("XDG_CACHE_HOME", home.join("cache"))
            .env("XDG_CONFIG_HOME", home.join("config"))
            .env("PATH", "/usr/local/bin:/usr/bin:/bin")
            .env("NO_COLOR", "1")
            .current_dir(home)
            .stdin(Stdio::null());
        command
    }

    fn output(&self, name: &str, home: &Path, args: &[&str]) -> Result<Vec<u8>, Error> {
        let out = self
            .command(name, home)
            .args(args)
            .output()
            .map_err(err(format_args!("run {}", self.program(name).display())))?;
        if !out.status.success() {
            return Err(Error(format!(
                "{name} {} ended with {}: {}",
                args.first().copied().unwrap_or_default(),
                out.status,
                tail(&out.stderr)
            )));
        }
        Ok(out.stdout)
    }

    /// Each program and its version, or why one does not answer.
    pub fn versions(&self) -> Result<Vec<String>, Error> {
        let home = Scratch::new()?;
        let mut versions = Vec::new();
        for (name, arg) in [
            ("betterleaks", "version"),
            ("trufflehog", "--version"),
            ("noseyparker", "--version"),
        ] {
            let out = self
                .command(name, &home.0)
                .arg(arg)
                .output()
                .map_err(err(format_args!("run {}", self.program(name).display())))?;
            if !out.status.success() {
                return Err(Error(format!("{name} {arg} ended with {}", out.status)));
            }
            let said = [out.stdout, out.stderr].concat();
            let said = String::from_utf8_lossy(&said);
            let first = said.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
            versions.push(format!(
                "{name} {}",
                first.trim().trim_start_matches(name).trim()
            ));
        }
        Ok(versions)
    }

    /// What each detector finds in each text, each value once a text, in
    /// the order of `texts`. Texts shorter than [`SHORTEST`] are not read.
    pub fn run<S: AsRef<str> + Sync>(&self, texts: &[S]) -> Result<Vec<Vec<Extra>>, Error> {
        let texts: Vec<&str> = texts.iter().map(AsRef::as_ref).collect();
        let mut found: Vec<Vec<Extra>> = vec![Vec::new(); texts.len()];
        if texts.iter().all(|text| text.trim().len() < SHORTEST) {
            return Ok(found);
        }
        let scratch = Scratch::new()?;
        let home = scratch.0.as_path();
        let files = home.join("texts");
        for (i, text) in texts.iter().enumerate() {
            if text.trim().len() >= SHORTEST {
                std::fs::write(files.join(format!("{i:07}.txt")), text.as_bytes())
                    .map_err(err("write a text to scan"))?;
            }
        }
        let dir = files
            .to_str()
            .ok_or_else(|| Error("the scratch directory's name".into()))?;
        let texts = texts.as_slice();
        // Side by side: each spends seconds starting before it reads a
        // byte, and none writes where another does. Their findings are
        // kept in one order whichever ends first.
        type Reads<'a> = &'a (dyn Fn(&mut [Vec<Extra>]) -> Result<(), Error> + Sync);
        let each: [Reads; 3] = [
            &|found| self.betterleaks(home, dir, texts, found),
            &|found| self.noseyparker(home, dir, texts, found),
            &|found| self.trufflehog(home, dir, texts, found),
        ];
        let read: Vec<Result<Vec<Vec<Extra>>, Error>> = std::thread::scope(|scope| {
            let running: Vec<_> = each
                .iter()
                .map(|reads| {
                    scope.spawn(move || {
                        let mut found = vec![Vec::new(); texts.len()];
                        reads(&mut found).map(|()| found)
                    })
                })
                .collect();
            running
                .into_iter()
                .map(|thread| {
                    thread
                        .join()
                        .unwrap_or_else(|_| Err(Error("a detector's thread panicked".into())))
                })
                .collect()
        });
        for one in read {
            for (all, mine) in found.iter_mut().zip(one?) {
                all.extend(mine);
            }
        }
        for extras in &mut found {
            let mut seen = HashSet::new();
            extras.retain(|extra| seen.insert(extra.value.clone()));
        }
        Ok(found)
    }

    fn betterleaks(
        &self,
        home: &Path,
        dir: &str,
        texts: &[&str],
        found: &mut [Vec<Extra>],
    ) -> Result<(), Error> {
        let report = home.join("betterleaks.json");
        let report_at = report
            .to_str()
            .ok_or_else(|| Error("the report's name".into()))?;
        self.output(
            "betterleaks",
            home,
            &[
                "dir",
                dir,
                "--no-banner",
                "--exit-code",
                "0",
                "--log-level",
                "error",
                "--report-format",
                "json",
                "--report-path",
                report_at,
            ],
        )?;
        // With nothing found it writes `null`.
        let written = std::fs::read(&report).map_err(err("read what betterleaks reported"))?;
        let findings: Option<Vec<Value>> = serde_json::from_slice(&written)
            .map_err(|e| Error(format!("read what betterleaks reported: {e}")))?;
        for finding in findings.unwrap_or_default() {
            let text = |key: &str| finding.get(key).and_then(Value::as_str).unwrap_or_default();
            let Some(i) = index_of(text("File"), texts.len()) else {
                return Err(Error(format!(
                    "betterleaks named a file that was not given: {}",
                    text("File")
                )));
            };
            let Some(value) = kept(text("Secret")) else {
                continue;
            };
            let rule = text("RuleID");
            let line = finding
                .get("StartLine")
                .and_then(Value::as_u64)
                .map(|n| n as usize);
            let (start, end) = locate(texts[i], &value, line);
            found[i].push(Extra {
                rule_id: format!("betterleaks.{rule}"),
                label: rule.replace(['-', '_'], " "),
                severity: generic(rule),
                start,
                end,
                value,
            });
        }
        Ok(())
    }

    fn trufflehog(
        &self,
        home: &Path,
        dir: &str,
        texts: &[&str],
        found: &mut [Vec<Extra>],
    ) -> Result<(), Error> {
        let exclude = self.exclude.join(",");
        let mut args = vec![
            "filesystem",
            dir,
            "--no-verification",
            "--no-update",
            "--json",
            "--log-level=-1",
        ];
        let excluded = format!("--exclude-detectors={exclude}");
        if !exclude.is_empty() {
            args.push(&excluded);
        }
        let out = self.output("trufflehog", home, &args)?;
        for line in String::from_utf8_lossy(&out).lines() {
            let Ok(finding) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            let Some(detector) = finding.get("DetectorName").and_then(Value::as_str) else {
                continue;
            };
            let at = finding.pointer("/SourceMetadata/Data/Filesystem");
            let file = at
                .and_then(|at| at.get("file"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            let Some(i) = index_of(file, texts.len()) else {
                return Err(Error(format!(
                    "trufflehog named a file that was not given: {file}"
                )));
            };
            let Some(value) = finding.get("Raw").and_then(Value::as_str).and_then(kept) else {
                continue;
            };
            let line = at
                .and_then(|at| at.get("line"))
                .and_then(Value::as_u64)
                .map(|n| n as usize);
            let (start, end) = locate(texts[i], &value, line);
            found[i].push(Extra {
                rule_id: format!("trufflehog.{}", detector.to_ascii_lowercase()),
                label: detector.to_owned(),
                // Unverified, as everything here is: its detectors lean on
                // verification to be sure.
                severity: "low",
                start,
                end,
                value,
            });
        }
        Ok(())
    }

    fn noseyparker(
        &self,
        home: &Path,
        dir: &str,
        texts: &[&str],
        found: &mut [Vec<Extra>],
    ) -> Result<(), Error> {
        let store = home.join("noseyparker");
        let store_at = store
            .to_str()
            .ok_or_else(|| Error("the datastore's name".into()))?;
        let quiet = ["--color", "never", "--progress", "never"];
        let mut scan = vec!["scan", "--datastore", store_at];
        scan.extend(quiet);
        scan.push(dir);
        self.output("noseyparker", home, &scan)?;
        let mut report = vec![
            "report",
            "--datastore",
            store_at,
            "--format",
            "jsonl",
            "--max-matches",
            "-1",
        ];
        report.extend(quiet);
        let out = self.output("noseyparker", home, &report)?;
        for line in String::from_utf8_lossy(&out).lines() {
            if line.trim().is_empty() {
                continue;
            }
            let finding: Value = serde_json::from_str(line)
                .map_err(|e| Error(format!("read what noseyparker reported: {e}")))?;
            let name = finding
                .get("rule_name")
                .and_then(Value::as_str)
                .unwrap_or("?");
            let rule = finding
                .get("rule_text_id")
                .and_then(Value::as_str)
                .unwrap_or(name);
            for matched in finding
                .get("matches")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let file = matched
                    .pointer("/provenance/0/path")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let Some(i) = index_of(file, texts.len()) else {
                    return Err(Error(format!(
                        "noseyparker named a file that was not given: {file}"
                    )));
                };
                let text = texts[i];
                let from = matched
                    .pointer("/location/offset_span/start")
                    .and_then(Value::as_u64)
                    .map_or(0, |n| n as usize)
                    .min(text.len());
                // Its rules capture what they matched in groups: the secret,
                // and for a connection string the user and host beside it.
                for group in matched
                    .get("groups")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                {
                    let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(group) else {
                        continue;
                    };
                    let Some(value) = kept(&String::from_utf8_lossy(&bytes)) else {
                        continue;
                    };
                    if self.skip.as_ref().is_some_and(|skip| skip(&value)) {
                        continue;
                    }
                    let (start, end) = match text.get(from..).and_then(|rest| rest.find(&value)) {
                        Some(at) => (from + at, from + at + value.len()),
                        None => locate(text, &value, None),
                    };
                    found[i].push(Extra {
                        rule_id: format!("noseyparker.{rule}"),
                        label: name.to_owned(),
                        severity: generic(name),
                        start,
                        end,
                        value,
                    });
                }
            }
        }
        Ok(())
    }
}

/// A scratch directory, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Result<Self, Error> {
        let path = std::env::temp_dir().join(format!(
            "dyson-secrets-{}-{}",
            std::process::id(),
            SCRATCH.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(path.join("texts"))
            .map_err(err(format_args!("make {}", path.display())))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))
                .map_err(err("keep the scratch directory private"))?;
        }
        Ok(Self(path))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The text a file in the scratch directory holds, from its name.
fn index_of(path: &str, count: usize) -> Option<usize> {
    let stem = Path::new(path).file_stem()?.to_str()?;
    stem.parse::<usize>().ok().filter(|i| *i < count)
}

/// Where a value is in a text: itself, else the line the detector named.
fn locate(text: &str, value: &str, line: Option<usize>) -> (usize, usize) {
    if let Some(start) = text.find(value) {
        return (start, start + value.len());
    }
    let Some(line) = line.filter(|n| *n >= 1) else {
        return (0, 0);
    };
    let mut start = 0;
    for (n, part) in text.split_inclusive('\n').enumerate() {
        if n + 1 == line {
            return (start, start + part.trim_end_matches(['\n', '\r']).len());
        }
        start += part.len();
    }
    (0, 0)
}

/// A detector's value as one secret is kept: without what its match ran
/// on into. TruffleHog cuts its input into pieces and a match at a piece's
/// end can stop inside a character (which reads back as U+FFFD); a match
/// in JSON-escaped text can end on the backslash of the escape after it.
fn kept(value: &str) -> Option<String> {
    let value = value.split('\u{fffd}').next().unwrap_or_default();
    let value = value.trim().trim_end_matches('\\').trim_end();
    (value.chars().count() >= SHORTEST && value.len() <= 64 * 1024).then(|| value.to_owned())
}

fn generic(name: &str) -> &'static str {
    if name.to_ascii_lowercase().starts_with("generic") {
        "low"
    } else {
        "medium"
    }
}

fn tail(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim();
    let from = text.char_indices().rev().nth(399).map_or(0, |(at, _)| at);
    text[from..].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_value_is_placed_by_itself_else_by_its_line() {
        let text = "one\ntwo KEY=abcdefgh\nthree";
        assert_eq!(locate(text, "abcdefgh", None), (12, 20));
        assert_eq!(locate(text, "elsewhere", Some(2)), (4, 20));
        assert_eq!(locate(text, "elsewhere", None), (0, 0));
    }

    #[test]
    fn a_value_is_kept_without_what_it_ran_on_into() {
        assert_eq!(kept("abcdefgh\\").as_deref(), Some("abcdefgh"));
        assert_eq!(kept("abcdefgh\u{fffd}xx").as_deref(), Some("abcdefgh"));
        assert_eq!(kept("short"), None);
    }

    #[test]
    fn files_are_read_back_to_their_texts() {
        assert_eq!(index_of("/tmp/x/texts/0000003.txt", 4), Some(3));
        assert_eq!(index_of("/tmp/x/texts/0000004.txt", 4), None);
        assert_eq!(index_of("/tmp/x/texts/other.txt", 4), None);
    }

    #[test]
    fn the_default_exclusions_are_read_from_the_file() {
        let external = External::new("/nonexistent");
        assert!(external.exclude.contains(&"JDBC".to_owned()));
        assert!(external.exclude.iter().all(|d| !d.starts_with('#')));
    }

    #[test]
    fn a_missing_program_is_an_error_not_an_empty_result() {
        let external = External::new("/nonexistent");
        assert!(external.run(&["a text long enough to be read"]).is_err());
        // Nothing worth reading needs no program.
        assert_eq!(external.run(&["short"]).unwrap(), vec![Vec::new()]);
    }
}
