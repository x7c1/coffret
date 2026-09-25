//! The line each [`Error`] variant is read out as; what went wrong under it is
//! the variant's `source`, and is not repeated here.

use std::fmt;

use coffret_usecase::root_marker::{MANAGEMENT_AREA, MARKER_FILE};

use super::{CreationStep, Error};

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLibraryName { name, defect } => {
                write!(f, "{name:?} cannot name a Library: {defect}")
            }
            Self::NoStateDirectory => f.write_str(
                "neither COFFRET_STATE_DIR, XDG_STATE_HOME, nor HOME is set, \
                 so there is nowhere to keep Libraries",
            ),
            Self::LibraryExists { name, path } => {
                write!(f, "the Library {name:?} is already at {}", path.display())
            }
            Self::NoSuchLibrary { name, path } => write!(
                f,
                "no Library {name:?} is on this device; nothing is at {}",
                path.display()
            ),
            // Which file it was and what was being done to it, in the
            // operation's own word. What the operating system answered is not
            // repeated here: it is the `io::Error` underneath, and a shell that
            // shows the chain prints it there — printing it inside this line as
            // well would say the whole refusal twice. The file is named because
            // this line is read by the person standing at the device with the
            // Library in front of them. Keeping a path out of a diagnostic
            // event is `redacted`'s job, not this one's (spec: EL-1).
            Self::Local(refused) => write!(
                f,
                "{} could not be {}",
                refused.path.display(),
                refused.operation
            ),
            Self::MalformedSettings { path, .. } => write!(
                f,
                "the settings at {} hold something this build cannot read",
                path.display()
            ),
            Self::UnsupportedSettingsVersion {
                path,
                version,
                expected,
            } => write!(
                f,
                "the settings at {} are version {version}, and this build reads version {expected}",
                path.display()
            ),
            Self::UnencodableSettings { path, .. } => write!(
                f,
                "the settings could not be encoded, so nothing was written to {}",
                path.display()
            ),
            Self::MasterKeyNotUnlocked { path, .. } => write!(
                f,
                "the Master Key at {} did not open; the Passphrase may not be the one it was \
                 written under",
                path.display()
            ),
            Self::KeyMaterial { .. } => {
                f.write_str("the key material a new Library is built from could not be produced")
            }
            // What the entropy source said is the value underneath, which a
            // shell showing the chain prints there; saying it inside this line
            // as well would spell one refusal twice over.
            Self::ServerKeyNotDrawn { .. } => {
                f.write_str("the key this server would admit its callers by could not be drawn")
            }
            // The Library and the process, and nothing about the key, the file
            // it is in, or the file the lock is on. Which file says a server is
            // running is this crate's own arrangement, and a person told to go
            // and delete one would be told to do the one thing that does not
            // help: the lock is the operating system's and is gone the moment
            // the process is (spec: LA-4, LA-8). What ends this state is
            // stopping that server, so that is what the sentence says — after a
            // semicolon and in the same clause-per-line voice every other
            // refusal here is written in, so that a shell printing the chain
            // reads one continuous line rather than a sentence of its own.
            Self::LibraryAlreadyServed { name, by } => {
                write!(
                    f,
                    "the Library {name:?} is already being served on this device"
                )?;
                if let Some(by) = by {
                    write!(f, ", by process {by}")?;
                }
                f.write_str(
                    "; one server at a time serves a Library, so stop that one before starting \
                     another",
                )
            }
            Self::MalformedStoragePrefix { .. } => {
                f.write_str("the Library has no place under the Storage prefix that was asked for")
            }
            Self::Index { .. } => f.write_str("the Library's catalog could not be used"),
            // "A step that reaches Drive" rather than "a call to Drive": what
            // this wraps includes the failure to build the HTTP client, which
            // reaches nothing. The head line has to stay true of every cause
            // printed under it.
            Self::Drive { .. } => f.write_str("a step that reaches Google Drive did not complete"),
            Self::NotADriveLibrary { name } => write!(
                f,
                "the Library {name:?} is not on Google Drive, so it has no grant to renew"
            ),
            Self::NotAuthorized { name, .. } => write!(
                f,
                "the Library {name:?} has no usable grant on Google Drive; \
                 run `coffret authorize --library {name}`"
            ),
            // What a name may be, in one sentence: a person told only which
            // character was wrong has to guess at the rest of the rule.
            Self::InvalidAccountName { name } => write!(
                f,
                "{name:?} cannot name an account: an account name is 1 to {} characters, each an \
                 ASCII letter, a digit, '-' or '_'",
                crate::account_name::AccountName::MAX_LEN
            ),
            Self::AccountNameRequired { held } => write!(
                f,
                "this device holds {held} accounts, so which one the Library uses has to be \
                 named: give --account with the name of one of them, or a new name to consent \
                 as another"
            ),
            Self::NoAccountReachesFolder => f.write_str(
                "none of the accounts this device holds reaches that folder, and a new account \
                 needs a name while this device holds any: give --account with a name for the \
                 account the folder is in",
            ),
            Self::ClientMismatch(mismatch) => write!(
                f,
                "the Library {:?} names the OAuth client {:?} and the account {:?} names {:?}; \
                 an account's grant is used only through the client it was consented to; give \
                 --account a new name to consent through the Library's client as another account",
                mismatch.library,
                mismatch.library_client,
                mismatch.account,
                mismatch.account_client
            ),
            Self::UnreadableAccountEnvelope {
                library,
                account,
                cause,
            } => write!(
                f,
                "the envelope that opens the account {account:?} for the Library {library:?} {}; \
                 nothing was renewed and no consent was asked for",
                match cause {
                    Some(_) => "could not be read",
                    None => "is missing",
                }
            ),
            Self::AccountNotOpened { account, library } => write!(
                f,
                "the account {account:?} opens only through a Library that references it, and \
                 the Passphrase given does not open one; the Passphrase of the Library \
                 {library:?} does"
            ),
            Self::PromotionNeedsName {
                library, account, ..
            } => write!(
                f,
                "the Library {library:?} keeps a grant of its own from an earlier build, and the \
                 account {account:?} this device already holds cannot take it in; name an account \
                 for it with `coffret authorize --library {library} --account NAME`"
            ),
            Self::NoSuchAccount { account } => {
                write!(f, "no account {account:?} is on this device")
            }
            Self::AccountFixed {
                library,
                account,
                requested,
            } => write!(
                f,
                "the Library {library:?} references the account {account:?}, not {requested:?}, \
                 and the account a Library references cannot be changed yet; run `coffret \
                 authorize --library {library}` to renew that account's grant"
            ),
            // The model's refusal quotes the prefix and says which part of the
            // shape went, and it is printed under this line rather than inside
            // it — a shell that shows the chain would otherwise say the whole
            // refusal twice.
            Self::MalformedMappingPrefix {
                prefix,
                cause: Some(_),
            } => write!(f, "{prefix:?} cannot be mapped"),
            Self::MalformedMappingPrefix {
                prefix,
                cause: None,
            } => write!(
                f,
                "{prefix:?} cannot be mapped: a mapping stands for one top-level component of \
                 the Library, and this names more than one"
            ),
            Self::NoSuchLocalRoot { path, .. } => {
                write!(f, "{} is not a directory on this device", path.display())
            }
            // Each of these says which folder it is about and what is standing
            // where, because that is the whole of what the person has to go and
            // look at — and each says nothing was recorded, since a mapping
            // half-recorded against a root with no identity is exactly what
            // none of them leaves behind. Where the thing a person would reach
            // for next is the run that just refused, the arm rules that out as
            // well: recording the mapping again, with a new identity asked for
            // or without, is not what gets a folder out of these states, and a
            // message that left it unsaid would have them spend a run finding
            // that out.
            // The name coffret keeps rather than the spelling on disk: a
            // case-folding volume may have handed the open a `.COFFRET` of the
            // person's, and a sentence saying `.coffret` is standing there would
            // send them looking for a name their folder does not hold
            // (spec: EP-14). Which spelling it wears changes nothing they do.
            Self::ManagementAreaNotADirectory { root } => write!(
                f,
                "the name coffret keeps for its own folder in {} — {MANAGEMENT_AREA}, or a \
                 spelling differing from it only in case — has something standing at it that is \
                 not a folder of coffret's; nothing was written and nothing was recorded",
                root.display()
            ),
            Self::ManagementAreaIncomplete { root } => write!(
                f,
                "{} holds a {MANAGEMENT_AREA} folder with no {MARKER_FILE} in it, which is what \
                 an interrupted registration leaves; nothing was written and nothing was \
                 recorded, and recording the mapping again meets this same refusal until that \
                 folder is out of the way",
                root.display()
            ),
            // The one of these about a folder that is the person's rather than
            // coffret's, so it is the one that names the folder to them — which
            // is the whole of what it has to offer, since coffret cannot tell
            // them which of the two the volume handed it. It says nothing about
            // an interrupted registration, because none of theirs was
            // interrupted, and it asks for a rename rather than for the folder
            // to be got out of the way.
            Self::ManagementAreaFolded { root, name } => write!(
                f,
                "{} holds a folder named {name}, which this filesystem does not tell apart from \
                 {MANAGEMENT_AREA}, the name coffret keeps for its own folder in a mapped root; \
                 nothing was written and nothing was recorded, and recording the mapping again \
                 meets this same refusal until that folder is renamed",
                root.display()
            ),
            Self::MarkerNotARegularFile { root } => write!(
                f,
                "{MANAGEMENT_AREA}/{MARKER_FILE} in {} is not a regular file; nothing was \
                 written and nothing was recorded, and a new identity asked for replaces one \
                 rather than repairing this",
                root.display()
            ),
            // What is wrong with the content is the reading's own answer and
            // travels as the cause, which a shell showing the chain prints
            // under this line rather than inside it as well.
            Self::MarkerMalformed { root, .. } => write!(
                f,
                "{MANAGEMENT_AREA}/{MARKER_FILE} in {} names no identity; nothing was \
                 written and nothing was recorded, and a new identity asked for replaces one \
                 rather than repairing this",
                root.display()
            ),
            // The one of these the marker's *reader* raises rather than its
            // writer, so it says what the others say with the tense changed:
            // nothing was placed, and recording that mapping again is the
            // gesture. Those words are the refusal's own rather than this
            // layer's, because a fetch that met the same state while placing the
            // rest of the Library owes a person the same ones, and a sentence
            // spelled out in both places is a sentence that can drift in one.
            Self::RootRefused(refusal) => write!(f, "{refusal}"),
            // The folder is named and the mapping is not sent for: nothing was
            // learned about it, so the one thing a person can act on is the
            // folder the read was refused in. Which of coffret's own names in it
            // refused comes from the refusal rather than being written out here,
            // because the marker is not the only one of them a reading passes
            // through: the folder holding it is refused on its own account, and
            // a sentence that named the marker for that would send a person to a
            // file whose own mode is perfectly sound. What the operating system
            // answered is the `io::Error` underneath, which a shell printing the
            // chain shows.
            Self::RootUnvouched { local_root, cause } => write!(
                f,
                "{} could not be checked against the mapping it was recorded for: {} in it \
                 could not be {}, and nothing was placed",
                local_root.display(),
                cause
                    .path
                    .strip_prefix(local_root)
                    .unwrap_or(&cause.path)
                    .display(),
                cause.operation
            ),
            // "No marker" rather than "nothing": the management area is made
            // before the identity that goes into it is drawn, so this is the one
            // of the five that may leave a folder of coffret's own behind — and
            // a person who reads that nothing happened and then meets
            // [`ManagementAreaIncomplete`](Self::ManagementAreaIncomplete) on
            // the next run has been told two things that cannot both be true.
            Self::RootMarkerNotDrawn { root, .. } => write!(
                f,
                "an identity for {} could not be drawn; no marker was written and nothing was \
                 recorded",
                root.display()
            ),
            Self::PassphraseNotGiven { .. } => f.write_str("no Passphrase was given"),
            Self::RecoveryCodeNotGiven { .. } => f.write_str("no Recovery Code was given"),
            // What went wrong is the cause's to say — the bucket may be absent,
            // the credentials refused, or the endpoint silent — and this says
            // only which bucket it was and that nothing came of it.
            Self::BucketUnreachable { bucket, .. } => write!(
                f,
                "the bucket {bucket:?} cannot hold a Library; nothing was created"
            ),
            Self::MalformedRecoveryCode { .. } => {
                f.write_str("what was entered is not a Recovery Code")
            }
            // Both halves of the rule, because one variant answers both flows
            // and the reader knows which one they are in: the folder name is
            // what Drive was asked about, and the trailing separator is the
            // likeliest way an S3 prefix ends up here — a prefix without it
            // satisfies everything the first half asks for, so a message that
            // stopped there would state a rule the person had already met.
            Self::NotALibraryFolder { location, .. } => write!(
                f,
                "{location:?} is not where a Library lives: a Library's own folder is named \
                 {:?} followed by sixteen hex characters, and on S3 its prefix is that name \
                 with a {:?} after it",
                coffret_model::LibraryId::APP_FOLDER_PREFIX,
                "/"
            ),
            Self::Sync { .. } => f.write_str("the sync did not finish"),
            Self::Freeze { .. } => f.write_str("the freeze did not finish"),
            Self::Fetch { .. } => f.write_str("the fetch did not finish"),
            // What this device would do with the file is not in it, because
            // nothing here was going to do anything with one: the question was
            // where the file belongs, and the answer is that there is not one.
            Self::LocalPathNotSettled { .. } => {
                f.write_str("where on this device that file belongs was not settled")
            }
            // Where it would have gone is not in it, because for some of these
            // there is nowhere it could have gone and for one of them nothing
            // was worked out at all. What a person handed over is the file, so
            // what did not happen to the file is the answer.
            Self::FileNotTakenIn { .. } => f.write_str("the file was not taken in"),
            // Which folder is not in it, the caller having just named one, and
            // neither is what was going to be done with what is in it: nothing
            // was, beyond showing it to whoever asked.
            Self::LocalFilesNotRead { .. } => {
                f.write_str("what this device has of its own there was not read")
            }
            // Which Entry is not in it, the caller having just named one, and
            // neither is what the bytes were wanted for: nothing here was going
            // to do anything with them but hand them over.
            Self::LocalFileNotOpened { .. } => {
                f.write_str("what this device has for that Entry was not opened")
            }
            Self::CatchUp { .. } => {
                f.write_str("the catalog was not brought to the Library's head")
            }
            Self::LibraryNotJoined { name, step, .. } => {
                write!(f, "the Library {name:?} was not joined: {step} failed")
            }
            Self::LibraryNotCreated {
                name,
                step,
                orphan_folder,
                ..
            } => {
                write!(f, "the Library {name:?} was not created: {step} failed")?;
                match orphan_folder {
                    Some(folder) => write!(
                        f,
                        "; the folder {folder:?} was created on Drive first and is still there"
                    ),
                    // The id is exactly what did not arrive, so where to look
                    // is the only thing left to say — and saying nothing would
                    // invite a second `init` that leaves two folders behind.
                    None if matches!(step, CreationStep::AppFolder) => f.write_str(
                        "; a folder may have been created before the answer was lost, so look \
                         for a `coffret-` folder on Drive before creating this Library again",
                    ),
                    None => Ok(()),
                }
            }
        }
    }
}
