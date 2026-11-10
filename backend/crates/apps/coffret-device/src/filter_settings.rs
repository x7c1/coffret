use serde::{Deserialize, Serialize};

/// The most characters a kept name may have.
///
/// Far past anything a person types into a search field, and short enough
/// that what is kept stays a few kilobytes at most: a longer one is refused
/// rather than cut, so what is kept is always what was typed (spec: LA-43).
pub const MOST_FILTER_NAME_CHARS: usize = 1_000;

/// The count of recent books a device that never kept one is answered with:
/// the one the explorer's panel offers at first.
const RECENT_AT_FIRST: u32 = 10;

/// What this device's explorer last had its tree filtered by: the name typed in
/// its search field, whether the most recent books were asked for, and how
/// many (spec: LA-43).
///
/// This device's own state, kept beside the Libraries rather than in one, and
/// never uploaded. See [`ExplorerFile`](crate::ExplorerFile) for where.
///
/// Every value of it is one the explorer could have been set to: a name of at
/// most [`MOST_FILTER_NAME_CHARS`] characters and a count from 1 to
/// `u32::MAX`. Both ways in — a page's request and the file read back — go
/// through the same check, so a file edited by hand into something else reads
/// as no filter rather than as a filter the explorer could not show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", try_from = "Unchecked")]
pub struct FilterSettings {
    name: String,
    recent_on: bool,
    recent: u32,
}

/// The settings as they arrive, before [`FilterSettings::new`] has looked at
/// them.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Unchecked {
    name: String,
    recent_on: bool,
    recent: u32,
}

impl TryFrom<Unchecked> for FilterSettings {
    type Error = &'static str;

    fn try_from(unchecked: Unchecked) -> Result<Self, Self::Error> {
        Self::new(unchecked.name, unchecked.recent_on, unchecked.recent)
    }
}

impl FilterSettings {
    /// No filter at all: no name, Recent off, and the count the panel offers
    /// at first, kept for when it is turned on.
    pub fn none() -> Self {
        Self {
            name: String::new(),
            recent_on: false,
            recent: RECENT_AT_FIRST,
        }
    }

    /// The settings, or a sentence saying why they are not ones the explorer
    /// could have been set to.
    pub fn new(name: String, recent_on: bool, recent: u32) -> Result<Self, &'static str> {
        if recent == 0 {
            return Err("the count of recent books is not a whole number from 1");
        }
        if name.chars().count() > MOST_FILTER_NAME_CHARS {
            return Err("the name is longer than 1,000 characters");
        }
        Ok(Self {
            name,
            recent_on,
            recent,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_count_of_none_is_refused() {
        assert!(FilterSettings::new(String::new(), true, 0).is_err());
        assert!(FilterSettings::new(String::new(), true, 1).is_ok());
    }

    #[test]
    fn a_name_past_the_bound_is_refused_counted_in_characters() {
        let longest = "あ".repeat(MOST_FILTER_NAME_CHARS);
        assert!(FilterSettings::new(longest.clone(), false, 10).is_ok());
        assert!(FilterSettings::new(format!("{longest}a"), false, 10).is_err());
    }

    #[test]
    fn the_wire_form_is_camel_case_and_checked() {
        let settings: FilterSettings =
            serde_json::from_str(r#"{"name":"cat","recentOn":true,"recent":5}"#)
                .expect("a whole setting reads");
        assert_eq!(
            settings,
            FilterSettings::new("cat".into(), true, 5).unwrap()
        );
        assert_eq!(
            serde_json::to_value(&settings).unwrap(),
            serde_json::json!({ "name": "cat", "recentOn": true, "recent": 5 }),
        );
        assert!(
            serde_json::from_str::<FilterSettings>(
                r#"{"name":"cat","recentOn":true,"recent":4294967295}"#
            )
            .is_ok(),
            "the largest count reads (spec: LA-43)"
        );
        for refused in [
            r#"{"name":"cat","recentOn":true,"recent":0}"#,
            r#"{"name":"cat","recentOn":true,"recent":2.5}"#,
            r#"{"name":"cat","recentOn":true,"recent":-1}"#,
            r#"{"name":"cat","recentOn":true,"recent":4294967296}"#,
            r#"{"name":"cat","recentOn":true}"#,
            r#"{"name":"cat","recentOn":true,"recent":5,"more":1}"#,
        ] {
            assert!(
                serde_json::from_str::<FilterSettings>(refused).is_err(),
                "{refused}"
            );
        }
    }
}
