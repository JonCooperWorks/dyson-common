//! Payment details in text: card numbers and IBANs.
//!
//! None of the secret detectors looks for either: they are not credentials
//! any of them knows an issuer for. What this finds is carried as a
//! [`Hit`](crate::Hit) like the rest, so it is listed, reviewed and
//! redacted the same way.
//!
//! Whether a run of digits is a card number is `card-validate`'s to say:
//! the brand its first digits name, a length that brand issues, and the
//! Luhn check digit. Whether a run of letters and digits is an IBAN is
//! `iban_validate`'s: the country's own format and the check digits. Only
//! where one may start and end is decided here.
//!
//! A card number is written whole (`4242424242424242`) or in the groups a
//! card prints it in, with one separator throughout
//! (`4242 4242 4242 4242`, `3782-822463-10005`). It is not part of a longer
//! number, a name, a decimal, a telephone number (`+49…`), an identifier
//! joined with hyphens or a row of bytes in hexadecimal. An IBAN is in
//! capitals, whole or in the groups of four it is printed in.
//!
//! A card's first six digits (its BIN) name who issued it. The table in
//! `data/` is compiled in, and a card found is tagged with what it says:
//! the bank, its country, credit or debit, and the card's category. Nothing
//! is asked of anyone: a lookup service would be told the numbers.
//!
//! One run of digits in ten passes a card's check digit by chance, and the
//! table lists nearly every prefix a scheme owns, so what makes a number a
//! card's is a bank named for its BIN, or a word for a card beside it (the
//! words Presidio's recogniser uses, and the fields a card is asked for
//! with): `high` with both, `medium` with one. With neither, a listed
//! number written in a card's groups is kept as `low`, and a bare one is
//! not a finding. An IBAN's format and check digits leave one chance in a
//! hundred to a string already shaped like one: it is `high`.

use std::{io::Read as _, sync::LazyLock};

use card_validate::{Type, Validate};
use iban::IbanLike as _;
use regex::Regex;

pub use crate::hit::Issuer;

/// One card number or IBAN in a text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// The rule it is reported under: `card.visa`, `card.amex`, `iban`.
    pub rule_id: String,
    /// What a listing calls it: `Visa card number`, `IBAN`.
    pub label: String,
    /// As written, separators and all.
    pub value: String,
    /// `high`, `medium` or `low` (see the module's words on it).
    pub severity: &'static str,
    /// A card's issuer, where the BIN table lists its first six digits; an
    /// IBAN's country.
    pub issuer: Option<Issuer>,
    /// Byte offsets in the text.
    pub start: usize,
    pub end: usize,
}

/// The BIN table: a line a BIN, in order.
static BINS: LazyLock<String> = LazyLock::new(|| {
    let packed: &[u8] = include_bytes!("../data/bins.tsv.gz");
    let mut table = String::new();
    flate2::read::GzDecoder::new(packed)
        .read_to_string(&mut table)
        .expect("the BIN table is compiled in whole");
    table
});
/// Where each line of the table starts.
static LINES: LazyLock<Vec<u32>> = LazyLock::new(|| {
    std::iter::once(0)
        .chain(BINS.match_indices('\n').map(|(at, _)| at as u32 + 1))
        .filter(|&at| (at as usize) < BINS.len())
        .collect()
});

/// What the BIN table says of a card number's first six digits.
pub fn issuer(digits: &str) -> Option<Issuer> {
    let bin = digits.get(..6)?;
    let line = |at: u32| BINS[at as usize..].split('\n').next().unwrap_or_default();
    let at = LINES
        .binary_search_by(|&at| line(at).get(..6).unwrap_or_default().cmp(bin))
        .ok()?;
    let mut fields = line(LINES[at]).split('\t').skip(1).map(str::to_owned);
    let mut next = || fields.next().unwrap_or_default();
    Some(Issuer {
        bin: bin.to_owned(),
        scheme: next(),
        funding: next(),
        category: next(),
        bank: next(),
        country: next(),
    })
}

/// Kept from one text: a table of test data holds thousands.
const MOST: usize = 200;
/// How far before and after a number a word for a card counts, in bytes.
const BEFORE: usize = 80;
const AFTER: usize = 40;

/// The groups a card number is printed in: sixteen digits in fours, an
/// American Express card's fifteen, a Diners Club card's fourteen, and the
/// thirteen and nineteen of older and newer cards.
const SHAPES: [&[usize]; 5] = [
    &[4, 4, 4, 4],
    &[4, 6, 5],
    &[4, 6, 4],
    &[4, 4, 4, 1],
    &[4, 4, 4, 4, 3],
];

/// A run of digits, single spaces and hyphens holding at least twelve
/// digits: the shortest card number.
static RUN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\d(?:[ -]?\d){11,}").expect("a valid pattern"));

/// Where an IBAN may start: a country, its check digits and the first of
/// what follows.
static IBAN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Z]{2}\d{2} ?[A-Z0-9]{4}").expect("a valid pattern"));
/// The shortest IBAN (Norway's) and the longest the standard allows.
const IBAN_SHORTEST: usize = 15;
const IBAN_LONGEST: usize = 34;

/// The words that say a number is a card's.
const WORDS: [&str; 24] = [
    "card",
    "credit",
    "debit",
    "visa",
    "mastercard",
    "master card",
    "amex",
    "american express",
    "discover",
    "jcb",
    "diners",
    "maestro",
    "unionpay",
    "cvv",
    "cvc",
    "expir",
    "billing",
    "tarjeta",
    "carte bancaire",
    "kreditkarte",
    "карт",
    "卡号",
    "信用卡",
    "银行卡",
];

/// Whether a rule is one of this detector's.
pub fn rule(id: &str) -> bool {
    id == "iban" || id.starts_with("card.")
}

/// Every card number and IBAN in `text`, each value once.
pub fn find(text: &str) -> Vec<Found> {
    let mut out = cards(text);
    for found in ibans(text) {
        if out.len() == MOST {
            break;
        }
        if !out.iter().any(|held| held.value == found.value) {
            out.push(found);
        }
    }
    out.sort_by_key(|found| found.start);
    out
}

/// The IBANs in `text`.
fn ibans(text: &str) -> Vec<Found> {
    let mut out: Vec<Found> = Vec::new();
    let bytes = text.as_bytes();
    let word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut from = 0;
    while let Some(lead) = IBAN.find_at(text, from) {
        let start = lead.start();
        from = start + 1;
        if start.checked_sub(1).is_some_and(|i| word(bytes[i])) {
            continue;
        }
        // Each place it could end: a country's IBAN has one length.
        let (mut at, mut held) = (start, 0);
        while let Some(&b) = bytes.get(at) {
            let spaced = b == b' ' && at > start && bytes[at - 1] != b' ';
            if !(b.is_ascii_uppercase() || b.is_ascii_digit() || spaced) {
                break;
            }
            at += 1;
            if b == b' ' {
                continue;
            }
            held += 1;
            if held > IBAN_LONGEST {
                break;
            }
            if held < IBAN_SHORTEST || bytes.get(at).is_some_and(|&next| word(next)) {
                continue;
            }
            let value = &text[start..at];
            if let Ok(iban) = value.parse::<iban::Iban>() {
                if !out.iter().any(|found| found.value == value) {
                    out.push(Found {
                        rule_id: "iban".to_owned(),
                        label: "IBAN".to_owned(),
                        value: value.to_owned(),
                        severity: "high",
                        issuer: Some(Issuer {
                            country: iban.country_code().to_owned(),
                            ..Issuer::default()
                        }),
                        start,
                        end: at,
                    });
                }
                from = at;
                break;
            }
        }
        if out.len() == MOST {
            break;
        }
    }
    out
}

/// The card numbers in `text`.
fn cards(text: &str) -> Vec<Found> {
    let mut out: Vec<Found> = Vec::new();
    let bytes = text.as_bytes();
    for run in RUN.find_iter(text) {
        // The groups of the run, each with the separator before it.
        let mut groups: Vec<(usize, usize, u8)> = Vec::new();
        let mut at = run.start();
        let mut before = 0u8;
        while at < run.end() {
            let from = at;
            while at < run.end() && bytes[at].is_ascii_digit() {
                at += 1;
            }
            groups.push((from, at, before));
            if at < run.end() {
                before = bytes[at];
                at += 1;
            }
        }
        let mut first = 0;
        while first < groups.len() {
            match number(text, &groups, first) {
                Some((last, kind)) => {
                    let (start, end) = (groups[first].0, groups[last].1);
                    let value = &text[start..end];
                    let digits: String = value.chars().filter(char::is_ascii_digit).collect();
                    let issuer = issuer(&digits);
                    let banked = issuer
                        .as_ref()
                        .is_some_and(|issuer| !issuer.bank.is_empty());
                    let worded = worded(text, start, end);
                    // A number the table names no bank for is one where the
                    // text says so, or where it is listed and written as a
                    // card is.
                    let kept = banked || worded || issuer.is_some() && first != last;
                    if kept && !out.iter().any(|found| found.value == value) {
                        out.push(Found {
                            rule_id: format!("card.{}", kind.name()),
                            label: format!("{} card number", brand(kind)),
                            value: value.to_owned(),
                            severity: match (banked, worded) {
                                (true, true) => "high",
                                (false, false) => "low",
                                _ => "medium",
                            },
                            issuer,
                            start,
                            end,
                        });
                        if out.len() == MOST {
                            return out;
                        }
                    }
                    first = last + 1;
                }
                None => first += 1,
            }
        }
    }
    out
}

/// The card number that starts at group `first` of a run, as the group it
/// ends at and its brand.
fn number(text: &str, groups: &[(usize, usize, u8)], first: usize) -> Option<(usize, Type)> {
    let bytes = text.as_bytes();
    let (start, _, joined) = groups[first];
    // Inside a run, it follows a space: `id-4242-…` is one longer name.
    let opens = match first {
        0 => start
            .checked_sub(1)
            .map(|i| bytes[i])
            .is_none_or(|b| !(b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'+' | b'-'))),
        _ => joined == b' ',
    };
    if !opens {
        return None;
    }
    let mut digits = String::new();
    let mut lengths: Vec<usize> = Vec::new();
    let mut best: Option<(usize, Type, usize)> = None;
    for (last, &(from, to, separator)) in groups.iter().enumerate().skip(first) {
        lengths.push(to - from);
        // In groups: the way a card prints them, one separator throughout.
        if last > first
            && (separator != groups[first + 1].2
                || !SHAPES.iter().any(|shape| shape.starts_with(&lengths)))
        {
            break;
        }
        digits.push_str(&text[from..to]);
        if digits.len() > 19 {
            break;
        }
        let grouped = last > first;
        let closes = match groups.get(last + 1) {
            Some(&(_, _, next)) => next == b' ',
            None => {
                let after = bytes.get(to).copied();
                let word = after.is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_');
                let joined = after == Some(b'-')
                    || after == Some(b'.') && bytes.get(to + 1).is_some_and(u8::is_ascii_digit);
                !word && !joined
            }
        };
        if !closes
            || digits.len() < 12
            || same(&digits)
            || grouped && !SHAPES.contains(&lengths.as_slice())
            || grouped && separator == b' ' && dumped(text, start, to)
        {
            continue;
        }
        if let Ok(valid) = Validate::from(&digits) {
            // Of the numbers that start here, the one of the commonest
            // length: a sixteen-digit number's first twelve pass the check
            // digit one time in ten.
            let rank = match digits.len() {
                16 => 0,
                15 => 1,
                14 => 2,
                other => other,
            };
            if best.is_none_or(|(_, _, held)| rank < held) {
                best = Some((last, valid.card_type, rank));
            }
        }
    }
    best.map(|(last, kind, _)| (last, kind))
}

/// Whether a number in groups of four stands among more groups of four:
/// bytes printed in hexadecimal (`4c44 5453 5442 4452 4557 4831`), not a
/// card.
fn dumped(text: &str, start: usize, end: usize) -> bool {
    let bytes = text.as_bytes();
    let word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let group = |part: &[u8]| part.len() == 4 && part.iter().all(u8::is_ascii_hexdigit);
    let before = start.checked_sub(5).is_some_and(|from| {
        bytes[start - 1] == b' '
            && group(&bytes[from..start - 1])
            && from.checked_sub(1).is_none_or(|i| !word(bytes[i]))
    });
    let after = bytes.get(end) == Some(&b' ')
        && bytes.get(end + 1..end + 5).is_some_and(group)
        && bytes.get(end + 5).is_none_or(|&b| !word(b));
    before || after
}

/// One digit throughout: `0000000000000000` passes the check digit.
fn same(digits: &str) -> bool {
    let first = digits.as_bytes()[0];
    digits.bytes().all(|b| b == first)
}

fn brand(kind: Type) -> &'static str {
    match kind {
        Type::VisaElectron => "Visa Electron",
        Type::Maestro => "Maestro",
        Type::Forbrugsforeningen => "Forbrugsforeningen",
        Type::Dankort => "Dankort",
        Type::Visa => "Visa",
        Type::MIR => "Mir",
        Type::MasterCard => "Mastercard",
        Type::Amex => "American Express",
        Type::DinersClub => "Diners Club",
        Type::Discover => "Discover",
        Type::UnionPay => "UnionPay",
        Type::JCB => "JCB",
        _ => "Payment",
    }
}

/// Whether a word for a card stands near the number.
fn worded(text: &str, start: usize, end: usize) -> bool {
    let mut from = start.saturating_sub(BEFORE);
    while !text.is_char_boundary(from) {
        from -= 1;
    }
    let mut to = (end + AFTER).min(text.len());
    while !text.is_char_boundary(to) {
        to += 1;
    }
    let near = text[from..to].to_lowercase();
    WORDS.iter().any(|word| near.contains(word))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(text: &str) -> Vec<String> {
        find(text).into_iter().map(|found| found.value).collect()
    }

    #[test]
    fn finds_a_number_whole_and_in_groups_and_names_its_brand() {
        let found = find("pay with 4242424242424242 or 3782-822463-10005, 5555 5555 5555 4444.");
        let told: Vec<_> = found
            .iter()
            .map(|f| (f.rule_id.as_str(), f.label.as_str(), f.value.as_str()))
            .collect();
        assert_eq!(
            told,
            [
                ("card.visa", "Visa card number", "4242424242424242"),
                (
                    "card.amex",
                    "American Express card number",
                    "3782-822463-10005"
                ),
                (
                    "card.mastercard",
                    "Mastercard card number",
                    "5555 5555 5555 4444"
                ),
            ]
        );
        let text = "x 4242424242424242";
        let found = &find(text)[0];
        assert_eq!(&text[found.start..found.end], "4242424242424242");
    }

    #[test]
    fn a_number_that_fails_the_check_digit_or_names_no_brand_is_not_one() {
        assert!(values("4242424242424241").is_empty());
        assert!(values("1234567812345670").is_empty());
        assert!(values("0000000000000000 4444444444444444").is_empty());
    }

    #[test]
    fn part_of_a_longer_number_or_name_is_not_a_card() {
        for text in [
            "0.4242424242424242",
            "4242424242424242.5",
            "id_4242424242424242",
            "4242424242424242abc",
            "+4242424242424242",
            "99994242424242424242",
            "77-4242-4242-4242-4242",
            "4242-4242-4242-4242-77-1",
            "42 42 42 42 42 42 42 42",
            "4242 4242-4242 4242",
            "fco_01a11bac-5018-7083-9340-eaa1763753af",
            "for n in 4000 16000 64000 256000; do",
            "-x 0 4853 4c44 5453 5442 4452 4557 4831 364e 5335",
            "3435 3637 5453 5442 4452 4557",
        ] {
            assert_eq!(values(text), Vec::<String>::new(), "{text}");
        }
    }

    #[test]
    fn what_follows_a_number_is_left_out_of_it() {
        assert_eq!(
            values("4242 4242 4242 4242 12/27 123"),
            ["4242 4242 4242 4242"]
        );
        assert_eq!(
            values("order 77 4242 4242 4242 4242 123 ok"),
            ["4242 4242 4242 4242"]
        );
        assert_eq!(values("4242424242424242 12 27"), ["4242424242424242"]);
        assert_eq!(values("It was 4242424242424242."), ["4242424242424242"]);
        assert_eq!(
            values("4242424242424242 4242424242424242 5555555555554444"),
            ["4242424242424242", "5555555555554444"]
        );
    }

    #[test]
    fn says_how_sure_it_is_by_its_bank_and_what_stands_beside_it() {
        let severity = |text: &str| find(text).first().map(|found| found.severity);
        // A bank is named for it.
        assert_eq!(severity("card_number: 4242424242424242"), Some("high"));
        assert_eq!(severity("4242424242424242 cvv 123"), Some("high"));
        assert_eq!(severity("卡号 4242424242424242"), Some("high"));
        assert_eq!(severity("4242 4242 4242 4242"), Some("medium"));
        assert_eq!(severity("4242424242424242"), Some("medium"));
        // Listed, with no bank named.
        assert_eq!(issuer("4000141234567893").unwrap().bank, "");
        assert_eq!(severity("pay by card 4000141234567893"), Some("medium"));
        assert_eq!(severity("4000 1412 3456 7893"), Some("low"));
        assert_eq!(severity("id 4000141234567893"), None);
        // Not listed at all.
        assert_eq!(issuer("5800051234567893"), None);
        assert_eq!(severity("card number 5800051234567893"), Some("medium"));
        assert_eq!(severity("5800 0512 3456 7893"), None);
        assert_eq!(severity("5800051234567893"), None);
    }

    #[test]
    fn finds_an_iban_whole_and_as_printed_and_names_its_country() {
        let text = "Pay DE44500105175407324931 (BIC INGDDEFFXXX) or \
                    GB82 WEST 1234 5698 7654 32 TODAY, iban=NO9386011117947.";
        let found = find(text);
        let told: Vec<_> = found
            .iter()
            .map(|f| {
                let country = f.issuer.as_ref().map(|issuer| issuer.country.as_str());
                (
                    f.rule_id.as_str(),
                    f.label.as_str(),
                    country,
                    f.value.as_str(),
                )
            })
            .collect();
        assert_eq!(
            told,
            [
                ("iban", "IBAN", Some("DE"), "DE44500105175407324931"),
                ("iban", "IBAN", Some("GB"), "GB82 WEST 1234 5698 7654 32"),
                ("iban", "IBAN", Some("NO"), "NO9386011117947"),
            ]
        );
        assert!(found.iter().all(|f| f.severity == "high"));
        assert_eq!(&text[found[0].start..found[0].end], found[0].value);
    }

    #[test]
    fn what_fails_the_check_digits_or_a_countrys_format_is_not_an_iban() {
        for text in [
            "DE44500105175407324932",
            "DE4450010517540732493",
            "XX44500105175407324931",
            "de44500105175407324931",
            "ADE44500105175407324931",
            "DE44500105175407324931X",
            "DE44500105175407324931_a",
            "SHA256E3B0C44298FC1C149AFBF4C8996FB924",
        ] {
            assert_eq!(values(text), Vec::<String>::new(), "{text}");
        }
    }

    #[test]
    fn names_a_cards_issuer_from_its_first_six_digits() {
        let found = &find("4242424242424242")[0];
        let named = found.issuer.as_ref().expect("a listed BIN");
        assert_eq!(
            (
                named.bin.as_str(),
                named.scheme.as_str(),
                named.funding.as_str()
            ),
            ("424242", "VISA", "CREDIT")
        );
        assert_eq!(
            (named.bank.as_str(), named.country.as_str()),
            ("STRIPE PAYMENTS UK, LTD.", "GB")
        );
        // The first and the last BIN of the table, and one it has not.
        assert!(issuer_of("002102").is_some_and(|i| i.country == "CN"));
        let last = BINS.trim_end().rsplit('\n').next().unwrap();
        assert!(issuer_of(&last[..6]).is_some());
        assert_eq!(issuer_of("000000"), None);
        assert_eq!(issuer("4242"), None);
        assert_eq!(LINES.len(), 374_788);
    }

    fn issuer_of(bin: &str) -> Option<Issuer> {
        issuer(&format!("{bin}0000000000"))
    }
}
