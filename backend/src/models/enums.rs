//! The closed string sets of the §5.3 contract as enums. The DB columns stay
//! TEXT and the wire/row fields stay `String`; these are parsed in one place
//! (validation with the strict `FromStr`, the rule cache with the lenient
//! `from_db`) and render back to the exact same strings (`as_str`).

use std::fmt;
use std::str::FromStr;

/// A string outside an enum's set (the caller supplies the 422 detail).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("unknown variant")]
pub struct UnknownVariant;

macro_rules! wire_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident => $wire:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            /// Every variant, in contract order.
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            /// The exact wire / DB string.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $($name::$variant => $wire),+
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        /// Strict, case-sensitive: only the exact wire strings parse.
        impl FromStr for $name {
            type Err = UnknownVariant;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($wire => Ok($name::$variant),)+
                    _ => Err(UnknownVariant),
                }
            }
        }
    };
}

wire_enum!(
    /// `chaos_mode` (OQ-2): a random 5xx, or an opt-in connection drop.
    ChaosMode { Error => "error", Dropout => "dropout" }
);

wire_enum!(
    /// `default_mode`: what an unmatched request gets.
    DefaultMode { Mock404 => "mock_404", Echo => "echo" }
);

wire_enum!(
    /// `served_by` on traces and `X-HookBox-Served-By`.
    ServedBy {
        Rule => "rule",
        Crud => "crud",
        Mitm => "mitm",
        Tunnel => "tunnel",
        Default => "default",
        Cors => "cors",
        Chaos => "chaos",
        RateLimit => "ratelimit",
    }
);

wire_enum!(
    /// A `body_conditions[].op`.
    BodyOp { Eq => "eq", Neq => "neq", Contains => "contains", Exists => "exists" }
);

wire_enum!(
    /// A `state_requirements[].op`.
    StateOp { Eq => "eq", Neq => "neq", Exists => "exists", Absent => "absent" }
);

impl ChaosMode {
    /// Lenient read of a stored value: `dropout` in any letter case is a
    /// drop, anything else the 5xx default (the engine's historical reading).
    pub fn from_db(raw: &str) -> Self {
        if raw.eq_ignore_ascii_case(ChaosMode::Dropout.as_str()) {
            ChaosMode::Dropout
        } else {
            ChaosMode::Error
        }
    }
}

impl DefaultMode {
    /// Lenient read of a stored value: exactly `echo` echoes, anything else
    /// is the `mock_404` default (the engine's historical reading).
    pub fn from_db(raw: &str) -> Self {
        if raw == DefaultMode::Echo.as_str() {
            DefaultMode::Echo
        } else {
            DefaultMode::Mock404
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `z.enum([...])` strings that follow `anchor` in the SPA schema.
    fn ts_enum(schemas: &str, anchor: &str) -> Vec<String> {
        let start = schemas.find(anchor).unwrap_or_else(|| panic!("{anchor}"));
        let rest = &schemas[start..];
        let open = rest.find("z.enum([").expect("z.enum") + "z.enum([".len();
        let close = rest[open..].find("])").expect("enum close");
        rest[open..open + close]
            .split(',')
            .map(|s| s.trim().trim_matches('\'').to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }

    fn wire<T: Copy>(all: &[T], as_str: fn(T) -> &'static str) -> Vec<String> {
        all.iter().map(|v| as_str(*v).to_string()).collect()
    }

    #[test]
    fn strings_match_the_spa_schema_enums() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/api/schemas.ts");
        let ts = std::fs::read_to_string(path).expect("read src/api/schemas.ts");
        let cases = [
            ("chaosModeSchema", wire(ChaosMode::ALL, ChaosMode::as_str)),
            (
                "defaultModeSchema",
                wire(DefaultMode::ALL, DefaultMode::as_str),
            ),
            ("servedBySchema", wire(ServedBy::ALL, ServedBy::as_str)),
            ("bodyConditionSchema", wire(BodyOp::ALL, BodyOp::as_str)),
            (
                "stateRequirementSchema",
                wire(StateOp::ALL, StateOp::as_str),
            ),
        ];
        for (anchor, rust) in cases {
            let anchor = format!("export const {anchor}");
            assert_eq!(ts_enum(&ts, &anchor), rust, "{anchor}");
        }
    }

    #[test]
    fn from_str_is_strict_and_round_trips() {
        for mode in DefaultMode::ALL {
            assert_eq!(mode.as_str().parse::<DefaultMode>(), Ok(*mode));
        }
        assert_eq!(DefaultMode::Mock404.to_string(), "mock_404");
        assert_eq!(ServedBy::RateLimit.as_str(), "ratelimit");
        assert!("Dropout".parse::<ChaosMode>().is_err());
        assert!("mock404".parse::<DefaultMode>().is_err());
        assert!("EQ".parse::<BodyOp>().is_err());
    }

    #[test]
    fn from_db_is_lenient_like_the_engine_always_was() {
        assert_eq!(ChaosMode::from_db("DROPOUT"), ChaosMode::Dropout);
        assert_eq!(ChaosMode::from_db("garbage"), ChaosMode::Error);
        assert_eq!(DefaultMode::from_db("echo"), DefaultMode::Echo);
        assert_eq!(DefaultMode::from_db("ECHO"), DefaultMode::Mock404);
    }
}
