//! The explorer's page as served in the set of colours this device chose
//! (spec: LA-38).
//!
//! The page asks the server what was chosen (`GET /api/appearance`), and until
//! it is answered the browser draws in the system's set: a person who chose
//! dark on a light system would see a light frame before the explorer is
//! drawn. So the host writes the choice into the page it serves — the
//! `<meta name="color-scheme">`, which the browser draws its own ground by
//! before anything of the page has run, and an attribute on the root that the
//! page starts from — and the first frame is already in the chosen set.
//! `System` is served as the build wrote it.

use std::borrow::Cow;

use coffret_device::Appearance;

/// What the build's `index.html` says the page may be drawn in: either set,
/// the system's choosing.
const EITHER: &str = r#"name="color-scheme" content="light dark""#;

/// The root element's opening, which the choice is written onto.
const ROOT: &str = "<html";

/// The attribute the page reads the served choice from
/// (`frontend/packages/apps/web/src/ShownAppearance.ts`).
const ATTRIBUTE: &str = "data-appearance";

/// `index` as served with `appearance` chosen: unchanged for `System`, and
/// otherwise with the colour scheme narrowed to the one chosen and the choice
/// written onto the root element.
///
/// A build that carries neither — a case's own, or one that dropped them — is
/// served as it is: how the explorer looks is never a reason for it not to
/// open.
pub(super) fn in_scheme(index: Cow<'static, [u8]>, appearance: Appearance) -> Cow<'static, [u8]> {
    let scheme = match appearance {
        Appearance::System => return index,
        Appearance::Light => "light",
        Appearance::Dark => "dark",
    };
    let Ok(text) = std::str::from_utf8(&index) else {
        return index;
    };
    let narrowed = text.replacen(
        EITHER,
        &format!(r#"name="color-scheme" content="{scheme}""#),
        1,
    );
    let marked = narrowed.replacen(ROOT, &format!(r#"{ROOT} {ATTRIBUTE}="{scheme}""#), 1);
    Cow::Owned(marked.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    const INDEX: &str = r#"<!doctype html>
<html lang="en">
  <head>
    <meta name="color-scheme" content="light dark" />
    <style>body { background: var(--color-ground, Canvas); }</style>
  </head>
</html>"#;

    fn served(appearance: Appearance) -> String {
        let bytes = in_scheme(Cow::Borrowed(INDEX.as_bytes()), appearance);
        String::from_utf8(bytes.into_owned()).expect("the page stays UTF-8")
    }

    // LA-38: a device that never chose, or chose the system's, is served the
    // page as the build wrote it, and the browser follows the system.
    #[test]
    fn the_system_s_choice_is_served_as_built() {
        assert_eq!(served(Appearance::System), INDEX);
    }

    // LA-38: dark chosen, the page says so before anything of it runs: the
    // browser draws its ground dark, and the page starts from dark.
    #[test]
    fn dark_is_written_into_the_page() {
        let page = served(Appearance::Dark);
        assert!(
            page.contains(r#"<meta name="color-scheme" content="dark" />"#),
            "{page}"
        );
        assert!(
            page.contains(r#"<html data-appearance="dark" lang="en">"#),
            "{page}"
        );
        assert!(!page.contains("light dark"), "{page}");
    }

    // LA-38: and light the same way, over a dark system.
    #[test]
    fn light_is_written_into_the_page() {
        let page = served(Appearance::Light);
        assert!(
            page.contains(r#"<meta name="color-scheme" content="light" />"#),
            "{page}"
        );
        assert!(
            page.contains(r#"<html data-appearance="light" lang="en">"#),
            "{page}"
        );
    }

    // The inline style is left exactly as it was, so the hash the policy
    // admits it by still holds.
    #[test]
    fn the_inline_style_is_untouched() {
        for appearance in [Appearance::Light, Appearance::Dark] {
            assert!(served(appearance)
                .contains("<style>body { background: var(--color-ground, Canvas); }</style>"));
        }
    }

    // A page that carries neither is served as it is.
    #[test]
    fn a_page_without_either_is_served_as_it_is() {
        let bare = "<title>index</title>";
        let bytes = in_scheme(Cow::Borrowed(bare.as_bytes()), Appearance::Dark);
        assert_eq!(&*bytes, bare.as_bytes());
    }
}
