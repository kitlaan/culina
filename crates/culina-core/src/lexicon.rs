//! Lexicons — the per-language word tables behind parse-or-note.
//!
//! One TOML file per language (`lexicon.en.toml`), channels per design.md
//! (Lexicons). Resolution order: document registry (`>> unit:`) →
//! collection overlay → tool core, selected by `>> lang:`.
//!
//! Overlay semantics are word-level: an overlay entry replaces the core
//! entry whole; list channels (approx, doneness, dates) union. Removal is
//! not supported — park that until a real collection needs it.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::error::{ParseError, Result};
use crate::fields;
use crate::units::Dimension;

const ENGLISH_CORE: &str = include_str!("../lexicons/lexicon.en.toml");

/// A loaded lexicon.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lexicon {
    /// Language tag; overlays omit it (they inherit the core's).
    #[serde(default)]
    pub lang: String,
    #[serde(default)]
    pub approx: WordList,
    #[serde(default)]
    pub doneness: WordList,
    #[serde(default)]
    pub dates: WordList,
    #[serde(default)]
    pub fuzzy: HashMap<String, FuzzyDuration>,
    #[serde(default)]
    pub verbs: HashMap<String, Verb>,
    #[serde(default)]
    pub units: HashMap<String, UnitDef>,
}

/// A bare `[section] words = [...]` channel.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct WordList {
    #[serde(default)]
    pub words: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct FuzzyDuration {
    /// Minimum of the range, in minutes.
    pub min: f64,
    /// Maximum of the range, in minutes.
    pub max: f64,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct Verb {
    /// Whether duration fields on this action become timers (default true).
    #[serde(default = "default_true")]
    pub timer: bool,
    /// base, 3rd person, past, participle.
    #[serde(default)]
    pub tense: Vec<String>,
    #[serde(default)]
    pub note: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct UnitDef {
    pub dim: Dimension,
    /// To the dimension base (grams, liters, meters, seconds); absent for
    /// countables and temperatures.
    #[serde(default)]
    pub factor: Option<f64>,
    #[serde(default)]
    pub short: Option<String>,
    #[serde(default)]
    pub long: Option<String>,
    /// Extra input spellings that are the same unit ("h" for "hour") — the
    /// key, short, and long already match.
    #[serde(default)]
    pub alt: Vec<String>,
    #[serde(default)]
    pub note: Option<String>,
}

impl<'de> serde::Deserialize<'de> for Dimension {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        // Trampoline through a shadow enum so Dimension keeps its
        // hand-written variants for the rest of the crate.
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "lowercase")]
        enum Dim {
            Mass,
            Volume,
            Length,
            Time,
            Temperature,
            Count,
        }
        match Dim::deserialize(d)? {
            Dim::Mass => Ok(Dimension::Mass),
            Dim::Volume => Ok(Dimension::Volume),
            Dim::Length => Ok(Dimension::Length),
            Dim::Time => Ok(Dimension::Time),
            Dim::Temperature => Ok(Dimension::Temperature),
            Dim::Count => Ok(Dimension::Count),
        }
    }
}

impl Lexicon {
    /// The embedded English core. Parsed once; the data is compiled in, so
    /// a bad core is a build bug and panics loudly.
    pub fn english() -> Lexicon {
        static CORE: OnceLock<Lexicon> = OnceLock::new();
        CORE.get_or_init(|| {
            Lexicon::from_toml(ENGLISH_CORE).expect("embedded English lexicon is valid")
        })
        .clone()
    }

    /// The core for a document's `>> lang:`. Only English ships today; other
    /// languages fall back to English until per-language cores exist.
    pub fn core_for(lang: &str) -> Lexicon {
        match lang {
            "en" | "" => Lexicon::english(),
            _ => Lexicon::english(),
        }
    }

    pub fn from_toml(text: &str) -> Result<Lexicon> {
        let lex: Lexicon = toml::from_str(text).map_err(|e| ParseError::BadLexicon {
            detail: e.to_string(),
        })?;
        Ok(lex)
    }

    pub fn from_path(path: &std::path::Path) -> Result<Lexicon> {
        let text = std::fs::read_to_string(path).map_err(|e| ParseError::BadLexicon {
            detail: format!("{}: {e}", path.display()),
        })?;
        Lexicon::from_toml(&text)
    }

    /// Overlay word-level: map entries replace whole, list channels union.
    pub fn overlay(mut self, over: &Lexicon) -> Lexicon {
        self.approx.words.extend(over.approx.words.iter().cloned());
        self.doneness
            .words
            .extend(over.doneness.words.iter().cloned());
        self.dates.words.extend(over.dates.words.iter().cloned());
        self.fuzzy
            .extend(over.fuzzy.iter().map(|(k, v)| (k.clone(), v.clone())));
        self.verbs
            .extend(over.verbs.iter().map(|(k, v)| (k.clone(), v.clone())));
        self.units
            .extend(over.units.iter().map(|(k, v)| (k.clone(), v.clone())));
        self
    }

    /// Document registry: `>> unit: verre = 90 ml` — the value defines the
    /// unit through existing ones (90 × ml's factor to liters). `line` is
    /// where the metadata line sits, for the loud error on a bad value.
    pub fn register_unit(&mut self, value: &str, line: usize) -> Result<()> {
        let bad = || ParseError::BadUnitRegistration {
            line,
            value: value.to_string(),
        };
        let (name, rhs) = value.split_once('=').ok_or_else(bad)?;
        let name = name.trim();
        if name.is_empty() {
            return Err(bad());
        }
        match fields::classify(self, rhs.trim()) {
            fields::FieldKind::Quantity(q) => {
                let factor = match (&q.unit, q.value) {
                    (Some(u), fields::Value::Single(n)) => u.factor.map(|f| f * n),
                    _ => None,
                };
                let def = UnitDef {
                    dim: q.unit.as_ref().map_or(Dimension::Other, |u| u.dim),
                    factor,
                    short: None,
                    long: None,
                    alt: Vec::new(),
                    note: None,
                };
                self.units.insert(name.to_string(), def);
                Ok(())
            }
            _ => Err(bad()),
        }
    }

    /// All input spellings a unit answers to — (spelling, canonical key,
    /// definition) — the key, short, long, and alt forms, each plus its `+s`
    /// plural. Used by the field grammar's longest match. One spelling must
    /// map to exactly one unit; cross-key duplicates are a lexicon bug.
    pub fn unit_spellings(&self) -> Vec<(String, String, &UnitDef)> {
        let mut out = Vec::new();
        for (key, def) in &self.units {
            let mut forms = vec![key.clone()];
            if let Some(s) = &def.short {
                forms.push(s.clone());
            }
            if let Some(l) = &def.long {
                forms.push(l.clone());
            }
            forms.extend(def.alt.iter().cloned());
            for form in forms {
                out.push((form.clone(), key.clone(), def));
                out.push((format!("{form}s"), key.clone(), def));
            }
        }
        out
    }

    pub fn verb(&self, word: &str) -> Option<&Verb> {
        self.verbs.get(word)
    }

    pub fn fuzzy(&self, word: &str) -> Option<&FuzzyDuration> {
        self.fuzzy.get(word)
    }
}
