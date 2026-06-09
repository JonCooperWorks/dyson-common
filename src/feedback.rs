//! Conversation feedback wire types.
//!
//! `dyson` writes these to disk per chat; `dyson-swarm`'s export path reads the
//! same JSON. The reaction-emoji mapping is shared by every input surface
//! (Telegram reactions, the web UI's emoji buttons) so it never drifts.

use serde::{Deserialize, Serialize};

/// A seven-point feedback rating, `Terrible` (-3) … `Excellent` (+3).
///
/// Serializes in `snake_case` (`"not_good"`, `"very_good"`, …) — this is the
/// canonical on-disk + export wire form that both repos persist and read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackRating {
    Terrible,
    Bad,
    NotGood,
    Decent,
    Good,
    VeryGood,
    Excellent,
}

impl FeedbackRating {
    /// Numeric score for this rating (-3 to +3).
    pub const fn score(self) -> i8 {
        match self {
            Self::Terrible => -3,
            Self::Bad => -2,
            Self::NotGood => -1,
            Self::Decent => 0,
            Self::Good => 1,
            Self::VeryGood => 2,
            Self::Excellent => 3,
        }
    }

    /// Map a reaction emoji to a rating, or `None` for an unrecognized emoji.
    pub fn from_emoji(emoji: &str) -> Option<Self> {
        match emoji {
            "💩" | "😡" | "🤮" => Some(Self::Terrible),
            "👎" => Some(Self::Bad),
            "😢" | "😐" => Some(Self::NotGood),
            "👍" | "👏" => Some(Self::Good),
            "🔥" | "🎉" | "😂" => Some(Self::VeryGood),
            "❤️" | "❤" | "🤯" | "💯" | "⚡" => Some(Self::Excellent),
            _ => None,
        }
    }
}

/// A single feedback entry linking a conversation turn to a rating.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackEntry {
    /// Index of the assistant message in the conversation's message list.
    pub turn_index: usize,
    /// The rating.
    pub rating: FeedbackRating,
    /// Numeric score (-3 to +3), denormalized for convenient export.
    pub score: i8,
    /// Unix timestamp (seconds) when recorded.
    pub timestamp: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_emoji_maps_known_reactions_and_rejects_others() {
        assert_eq!(FeedbackRating::from_emoji("👍"), Some(FeedbackRating::Good));
        assert_eq!(
            FeedbackRating::from_emoji("💯"),
            Some(FeedbackRating::Excellent)
        );
        assert_eq!(FeedbackRating::from_emoji("🦀"), None);
        assert_eq!(FeedbackRating::from_emoji(""), None);
    }

    #[test]
    fn score_matches_variant() {
        assert_eq!(FeedbackRating::Terrible.score(), -3);
        assert_eq!(FeedbackRating::Decent.score(), 0);
        assert_eq!(FeedbackRating::Excellent.score(), 3);
    }

    #[test]
    fn rating_serializes_snake_case() {
        // The on-disk + export wire contract both repos depend on.
        assert_eq!(
            serde_json::to_string(&FeedbackRating::NotGood).unwrap(),
            "\"not_good\""
        );
        assert_eq!(
            serde_json::to_string(&FeedbackRating::VeryGood).unwrap(),
            "\"very_good\""
        );
        assert_eq!(
            serde_json::from_str::<FeedbackRating>("\"good\"").unwrap(),
            FeedbackRating::Good
        );
    }

    #[test]
    fn entry_round_trips() {
        let e = FeedbackEntry {
            turn_index: 4,
            rating: FeedbackRating::VeryGood,
            score: FeedbackRating::VeryGood.score(),
            timestamp: 1_700_000_000,
        };
        let json = serde_json::to_string(&e).unwrap();
        let back: FeedbackEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.turn_index, 4);
        assert_eq!(back.rating, FeedbackRating::VeryGood);
        assert_eq!(back.score, 2);
        assert_eq!(back.timestamp, 1_700_000_000);
    }
}
