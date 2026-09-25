//! What a diagnostic event may be told of an [`Error`]: which state of this
//! device it is, and none of the names it is about.

use coffret_model::Redacted;

use super::Error;

impl Redacted for Error {
    /// Which state of this device it is, and none of the names it is about.
    ///
    /// Almost every variant here is *identified* by something a person chose —
    /// the Library's name, the directory it occupies, the settings file, the
    /// bucket, the mapping prefix, the folder on Drive — because that is what
    /// makes the message useful to the one reader it is written for, who is
    /// standing at this device with the Library in front of them. None of it
    /// may be written down, so what a diagnostic event gets is the variant,
    /// the step-shaped facts around it, and the redacted cause underneath.
    ///
    /// Four causes deliberately stop here rather than going underneath.
    /// [`Drive`](Self::Drive) and [`NotAuthorized`](Self::NotAuthorized) carry
    /// the Drive gateway's own failure, which records itself where it happens
    /// with the bodies and URLs already redacted — and which, in the token
    /// cache's case, names a file on this device.
    /// [`PassphraseNotGiven`](Self::PassphraseNotGiven) and
    /// [`RecoveryCodeNotGiven`](Self::RecoveryCodeNotGiven) carry whatever the
    /// terminal or the explorer reported, which are boxed errors this layer
    /// knows nothing about. In all four the identity is what the log is for.
    ///
    /// [`ServerKeyNotDrawn`](Self::ServerKeyNotDrawn) goes the other way and
    /// writes its cause down as it stands, for the reason
    /// `coffret_format::Error` writes the same value down: what `getrandom`
    /// prints is about this machine's random source and names no path, no
    /// filename and nothing anybody chose (spec: EL-3, EL-4). Since the
    /// Library this key was for may not go with it, that is the whole of what a
    /// reader of this event can act on.
    fn redacted(&self) -> String {
        match self {
            Self::InvalidLibraryName { defect, .. } => {
                format!("Device::InvalidLibraryName(defect={defect})")
            }
            Self::NoStateDirectory => "Device::NoStateDirectory".to_owned(),
            Self::LibraryExists { .. } => "Device::LibraryExists".to_owned(),
            Self::NoSuchLibrary { .. } => "Device::NoSuchLibrary".to_owned(),
            Self::Local(refused) => format!("Device::Local: {}", refused.redacted()),
            Self::MalformedSettings { .. } => "Device::MalformedSettings".to_owned(),
            Self::UnsupportedSettingsVersion {
                version, expected, ..
            } => format!(
                "Device::UnsupportedSettingsVersion(version={version}, expected={expected})"
            ),
            Self::UnencodableSettings { .. } => "Device::UnencodableSettings".to_owned(),
            Self::MasterKeyNotUnlocked { cause, .. } => {
                format!("Device::MasterKeyNotUnlocked: {}", cause.redacted())
            }
            Self::KeyMaterial { cause } => {
                format!("Device::KeyMaterial: {}", cause.redacted())
            }
            Self::ServerKeyNotDrawn { cause } => format!("Device::ServerKeyNotDrawn: {cause}"),
            // The process number and not the Library's name. A process id is
            // the operating system's own and names nothing a person chose, so
            // it is evidence a diagnostic event may keep — and it is the one
            // fact worth keeping here, since what a reader of this event wants
            // to know is which two runs were racing (spec: EL-1).
            Self::LibraryAlreadyServed { by, .. } => format!(
                "Device::LibraryAlreadyServed(by={})",
                match by {
                    Some(by) => by.to_string(),
                    None => "unknown".to_owned(),
                }
            ),
            Self::MalformedStoragePrefix { cause } => {
                format!("Device::MalformedStoragePrefix: {}", cause.redacted())
            }
            Self::Index { cause } => format!("Device::Index: {}", cause.redacted()),
            Self::Drive { .. } => "Device::Drive".to_owned(),
            Self::NotADriveLibrary { .. } => "Device::NotADriveLibrary".to_owned(),
            Self::NotAuthorized { cause, .. } => format!(
                "Device::NotAuthorized(cached={})",
                match cause {
                    Some(_) => "unreadable",
                    None => "absent",
                }
            ),
            // The prefix is a folder somebody means to keep their files in, so
            // it is Library content and stays out of the diagnostic event;
            // what is left is which of the two rules it missed.
            Self::MalformedMappingPrefix { cause, .. } => match cause {
                Some(cause) => format!("Device::MalformedMappingPrefix: {}", cause.redacted()),
                None => "Device::MalformedMappingPrefix(more than one component)".to_owned(),
            },
            Self::NoSuchLocalRoot { cause, .. } => format!(
                "Device::NoSuchLocalRoot(kind={})",
                match cause {
                    Some(cause) => format!("{:?}", cause.kind()),
                    None => "none".to_owned(),
                }
            ),
            // The root is a folder somebody keeps their own files in, so it is
            // Library content and stays out of the event. Which state of the
            // management area it was does not name anything of theirs — it is
            // coffret's own vocabulary about coffret's own folder — so it stays.
            Self::ManagementAreaNotADirectory { .. } => {
                "Device::ManagementAreaNotADirectory".to_owned()
            }
            Self::ManagementAreaIncomplete { .. } => "Device::ManagementAreaIncomplete".to_owned(),
            // The root is the person's folder and so is the name standing in
            // it — this is the one of these refusals about a folder of theirs
            // rather than coffret's, so the name a person reads in the message
            // is exactly what an event must not carry (spec: EL-1).
            Self::ManagementAreaFolded { .. } => "Device::ManagementAreaFolded".to_owned(),
            Self::MarkerNotARegularFile { .. } => "Device::MarkerNotARegularFile".to_owned(),
            Self::MarkerMalformed { cause, .. } => {
                format!("Device::MarkerMalformed(defect={})", cause.defect())
            }
            // Which shape the wrong folder took, and neither the folder nor
            // either identity: the reason is coffret's own vocabulary about
            // coffret's own file (spec: EL-1).
            Self::RootRefused(refusal) => {
                format!("Device::RootRefused: {}", refusal.reason.redacted())
            }
            // Which operation the disk refused and what sort of refusal it was,
            // and neither the root nor the file under it: the folder is the
            // person's own name for it (spec: EL-1), and the identity here is
            // the variant, which is what tells a reader counting these that the
            // marker went unread rather than that a mapping is wrong.
            Self::RootUnvouched { cause, .. } => {
                format!("Device::RootUnvouched: {}", cause.redacted())
            }
            Self::RootMarkerNotDrawn { cause, .. } => {
                format!("Device::RootMarkerNotDrawn: {}", cause.redacted())
            }
            Self::PassphraseNotGiven { .. } => "Device::PassphraseNotGiven".to_owned(),
            Self::RecoveryCodeNotGiven { .. } => "Device::RecoveryCodeNotGiven".to_owned(),
            Self::BucketUnreachable { cause, .. } => {
                format!("Device::BucketUnreachable: {}", cause.redacted())
            }
            Self::MalformedRecoveryCode { cause } => {
                format!("Device::MalformedRecoveryCode: {}", cause.redacted())
            }
            Self::NotALibraryFolder { .. } => "Device::NotALibraryFolder".to_owned(),
            Self::Sync { cause } => format!("Device::Sync: {}", cause.redacted()),
            Self::Freeze { cause } => format!("Device::Freeze: {}", cause.redacted()),
            Self::Fetch { cause } => format!("Device::Fetch: {}", cause.redacted()),
            Self::LocalPathNotSettled { cause } => {
                format!("Device::LocalPathNotSettled: {}", cause.redacted())
            }
            Self::FileNotTakenIn { cause } => {
                format!("Device::FileNotTakenIn: {}", cause.redacted())
            }
            Self::LocalFilesNotRead { cause } => {
                format!("Device::LocalFilesNotRead: {}", cause.redacted())
            }
            Self::LocalFileNotOpened { cause } => {
                format!("Device::LocalFileNotOpened: {}", cause.redacted())
            }
            Self::CatchUp { cause } => format!("Device::CatchUp: {}", cause.redacted()),
            Self::LibraryNotCreated {
                step,
                orphan_folder,
                cause,
                ..
            } => format!(
                "Device::LibraryNotCreated(step={step:?}, orphan_folder={}): {}",
                orphan_folder.is_some(),
                cause.redacted(),
            ),
            Self::LibraryNotJoined { step, cause, .. } => format!(
                "Device::LibraryNotJoined(step={step:?}): {}",
                cause.redacted()
            ),
        }
    }
}
