use clap::Args;
use coffret_device::{LibraryId, LibraryRef};
pub use coffret_shell::which_library::library_id;

/// Which Library on this device a command acts on: by its name, or by its ID,
/// exactly one of the two (see [`coffret_shell::which_library`]).
#[derive(Args)]
#[group(required = true, multiple = false)]
pub struct WhichLibrary {
    /// The Library on this device to act on, by its name. Where more than one
    /// Library on this device carries the name, the command is refused with
    /// their IDs, and --library-id says which one
    #[arg(long)]
    pub library: Option<String>,
    /// The Library on this device to act on, by its ID: the 16 lowercase hex
    /// characters `init` and `join` print
    #[arg(long, value_parser = library_id)]
    pub library_id: Option<LibraryId>,
}

impl WhichLibrary {
    /// The Library the flags name.
    pub fn library(&self) -> LibraryRef {
        coffret_shell::which_library::library(self.library.clone(), self.library_id)
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[derive(Parser)]
    struct Command {
        #[command(flatten)]
        which: WhichLibrary,
    }

    fn parsed(typed: &[&str]) -> Result<LibraryRef, clap::Error> {
        Command::try_parse_from(std::iter::once("coffret").chain(typed.iter().copied()))
            .map(|command| command.which.library())
    }

    // A name and an ID are told apart by the flag that carries them, so a name
    // that spells an ID is still a name.
    #[test]
    fn a_library_is_named_by_one_flag_or_the_other() {
        assert_eq!(
            parsed(&["--library", "books"]).unwrap(),
            LibraryRef::named("books")
        );
        assert_eq!(
            parsed(&["--library", "0123456789abcdef"]).unwrap(),
            LibraryRef::named("0123456789abcdef")
        );
        assert_eq!(
            parsed(&["--library-id", "0123456789abcdef"]).unwrap(),
            LibraryRef::Id(LibraryId::from_hex("0123456789abcdef").unwrap())
        );
    }

    // Exactly one: both is refused, and so is neither, and so is an ID that is
    // not one.
    #[test]
    fn both_neither_or_a_malformed_id_is_refused() {
        for typed in [
            &["--library", "books", "--library-id", "0123456789abcdef"][..],
            &[][..],
            &["--library-id", "books"][..],
        ] {
            assert!(parsed(typed).is_err(), "{typed:?} must be refused");
        }
    }
}
