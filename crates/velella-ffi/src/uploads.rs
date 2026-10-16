//! Attachment upload across FFI (composer plan rev 1.8, slice CU): the attachment descriptor, the
//! limits check, the send with attachments and its nonce-keyed cancel, retry and discard, and the
//! HEIC / JXR conversion config. The pending send itself is core's echo row, delivered in the
//! message snapshot (`MessageRow::upload`).
//!
//! No logging (CU-7): a descriptor is an object whose `Debug` and `Display` show nothing of its
//! path, filename or description, and Swift's mirror of it is empty (Transport/Attachments.swift).
use crate::transport::UploadFile;
use crate::{runtime, SessionHandle};
use std::fmt;
use std::sync::Arc;
use velella_core as core;

/// One attached file: its verified identity (the app's attach-time check, CU-6), the filename
/// the message shows (the slot uses the file's own name), alt text (sent whenever set, `""`
/// included), spoiler, the original's MIME type (`""` when unknown, CU-5) and, for a converted
/// HEIC / JXR, the original's MD5 in lowercase hex (dropped unless valid, D16a).
#[derive(uniffi::Object)]
#[uniffi::export(Debug, Display)]
pub struct AttachmentDescriptor(pub(crate) core::AttachmentDescriptor);

impl fmt::Debug for AttachmentDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for AttachmentDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AttachmentDescriptor([REDACTED])")
    }
}

#[uniffi::export]
impl AttachmentDescriptor {
    #[uniffi::constructor]
    pub fn new(
        file: Arc<UploadFile>,
        filename: String,
        description: Option<String>,
        spoiler: bool,
        content_type: String,
        original_md5: Option<String>,
    ) -> Arc<Self> {
        Arc::new(Self(core::AttachmentDescriptor {
            file: file.0.clone(),
            filename,
            description,
            spoiler,
            content_type,
            original_md5,
        }))
    }

    pub fn file(&self) -> Arc<UploadFile> {
        Arc::new(UploadFile(self.0.file.clone()))
    }
    pub fn filename(&self) -> String {
        self.0.filename.clone()
    }
    /// The alt text (`description` on the wire; Swift's `description` is the redacted text).
    pub fn alt_text(&self) -> Option<String> {
        self.0.description.clone()
    }
    pub fn spoiler(&self) -> bool {
        self.0.spoiler
    }
    pub fn content_type(&self) -> String {
        self.0.content_type.clone()
    }
    pub fn original_md5(&self) -> Option<String> {
        self.0.original_md5.clone()
    }
    pub fn size(&self) -> u64 {
        self.0.file.size
    }
}

/// When a limits check runs (OA A36).
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum LimitCheckTime {
    Add,
    Send,
}

/// The limit a set of attachments breaks; the UI shows the official alert for each (CUA).
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum AttachmentLimitError {
    TooManyFiles { limit: u32 },
    EmptyFile,
    FileTooLarge { limit: u64 },
    TotalTooLarge { limit: u64 },
}

impl From<core::LimitError> for AttachmentLimitError {
    fn from(e: core::LimitError) -> Self {
        match e {
            core::LimitError::TooManyFiles { limit } => Self::TooManyFiles {
                limit: u32::try_from(limit).unwrap_or(u32::MAX),
            },
            core::LimitError::EmptyFile => Self::EmptyFile,
            core::LimitError::FileTooLarge { limit } => Self::FileTooLarge { limit },
            core::LimitError::TotalTooLarge { limit } => Self::TotalTooLarge { limit },
        }
    }
}

/// The outcome of `send_with_attachments`: started (the row and calls go by `nonce`), or refused
/// with nothing sent or queued.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum AttachmentSendResult {
    Started {
        nonce: u64,
    },
    NoFiles,
    Limit {
        error: AttachmentLimitError,
    },
    /// A guild channel where ATTACH_FILES is denied or unknown.
    NotAllowed,
    /// Before READY.
    NotReady,
    /// Not the open channel.
    NotOpen,
}

/// Why a pending send failed; `MessageRow::failure` carries its fixed text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum UploadFailureReason {
    Unauthorized,
    Captcha,
    RateLimited,
    Network,
    Forbidden,
    TooLarge,
    /// Discord named an upload host other than the one D13a allows; nothing was sent there.
    HostRefused,
    /// The file changed or is gone since it was attached; nothing was sent.
    FileChanged,
    PreflightRefused,
    UploadFailed,
    BadResponse,
    Failed,
}

impl From<core::UploadFailure> for UploadFailureReason {
    fn from(f: core::UploadFailure) -> Self {
        use core::UploadFailure as F;
        match f {
            F::Unauthorized => Self::Unauthorized,
            F::Captcha => Self::Captcha,
            F::RateLimited => Self::RateLimited,
            F::Network => Self::Network,
            F::Forbidden => Self::Forbidden,
            F::TooLarge => Self::TooLarge,
            F::HostRefused => Self::HostRefused,
            F::FileChanged => Self::FileChanged,
            F::PreflightRefused => Self::PreflightRefused,
            F::UploadFailed => Self::UploadFailed,
            F::BadResponse => Self::BadResponse,
            F::Failed => Self::Failed,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum UploadOutcome {
    Sending,
    Failed {
        reason: UploadFailureReason,
    },
    /// Cancelled before the message create: put the row's text, reply target and files back into
    /// the composer, then discard the row (D14a).
    Cancelled,
}

#[derive(uniffi::Record)]
pub struct PendingUploadFile {
    pub descriptor: Arc<AttachmentDescriptor>,
    /// Bytes the upload host has.
    pub sent: u64,
    pub size: u64,
}

/// The attachment part of a pending send's row: the single source the UI draws (CU FFI).
#[derive(uniffi::Record)]
pub struct PendingUpload {
    pub files: Vec<PendingUploadFile>,
    pub reply_to: Option<u64>,
    /// Shift-drop's send.
    pub instant: bool,
    pub outcome: UploadOutcome,
}

impl From<core::PendingUpload> for PendingUpload {
    fn from(p: core::PendingUpload) -> Self {
        Self {
            files: p
                .files
                .into_iter()
                .map(|f| PendingUploadFile {
                    size: f.descriptor.file.size,
                    descriptor: Arc::new(AttachmentDescriptor(f.descriptor)),
                    sent: f.sent,
                })
                .collect(),
            reply_to: p.reply_to,
            instant: p.instant,
            outcome: match p.state {
                core::UploadState::Sending => UploadOutcome::Sending,
                core::UploadState::Failed(f) => UploadOutcome::Failed { reason: f.into() },
                core::UploadState::Cancelled => UploadOutcome::Cancelled,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ImageConversionKind {
    Heic,
    Jxr,
}

/// The account's conversion config (D16a). `Unknown` and `Disabled` upload the original.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ImageConversionConfig {
    Unknown,
    Disabled,
    Enabled {
        quality: u8,
        max_file_size_bytes: u64,
    },
}

#[uniffi::export]
impl SessionHandle {
    /// The attachment limits over the files' sizes, for the conversation `channel_id`; nil when
    /// none is broken. On add, an account with premium is checked only for count and emptiness.
    pub fn attachment_limits(
        &self,
        channel_id: u64,
        sizes: Vec<u64>,
        at: LimitCheckTime,
    ) -> Option<AttachmentLimitError> {
        let at = match at {
            LimitCheckTime::Add => core::LimitCheck::Add,
            LimitCheckTime::Send => core::LimitCheck::Send,
        };
        self.0
            .attachment_limits(channel_id, &sizes, at)
            .err()
            .map(Into::into)
    }

    /// Sends `attachments` with `content` (may be empty) to the open channel: permission and
    /// limits first, then the pending row (the echo, by nonce) and the uploads. `instant`:
    /// Shift-drop's send (`instant_upload`, empty content, the draft left alone).
    #[uniffi::method(default(reply_to = None, instant = false))]
    pub fn send_with_attachments(
        &self,
        channel_id: u64,
        content: String,
        attachments: Vec<Arc<AttachmentDescriptor>>,
        reply_to: Option<u64>,
        instant: bool,
    ) -> AttachmentSendResult {
        let _context = runtime().enter();
        let files = attachments.iter().map(|a| a.0.clone()).collect();
        match self
            .0
            .send_with_attachments(channel_id, content, reply_to, files, instant)
        {
            Ok(nonce) => AttachmentSendResult::Started { nonce },
            Err(core::SendRefusal::NoFiles) => AttachmentSendResult::NoFiles,
            Err(core::SendRefusal::Limits(e)) => AttachmentSendResult::Limit { error: e.into() },
            Err(core::SendRefusal::NotAllowed) => AttachmentSendResult::NotAllowed,
            Err(core::SendRefusal::NotReady) => AttachmentSendResult::NotReady,
            Err(core::SendRefusal::NotOpen) => AttachmentSendResult::NotOpen,
        }
    }

    /// Cancels send `nonce` unless its message create was dispatched; false otherwise.
    pub fn cancel_upload(&self, nonce: u64) -> bool {
        let _context = runtime().enter();
        self.0.cancel_upload(nonce)
    }

    /// Runs failed send `nonce` again from the slot requests.
    pub fn retry_upload(&self, nonce: u64) -> bool {
        let _context = runtime().enter();
        self.0.retry_upload(nonce)
    }

    /// Removes failed or cancelled send `nonce`'s row.
    pub fn discard_upload(&self, nonce: u64) -> bool {
        self.0.discard_upload(nonce)
    }

    pub fn image_conversion_config(&self, kind: ImageConversionKind) -> ImageConversionConfig {
        let kind = match kind {
            ImageConversionKind::Heic => core::ConversionKind::Heic,
            ImageConversionKind::Jxr => core::ConversionKind::Jxr,
        };
        match self.0.upload_conversion(kind) {
            core::ConversionConfig::Unknown => ImageConversionConfig::Unknown,
            core::ConversionConfig::Disabled => ImageConversionConfig::Disabled,
            core::ConversionConfig::Enabled {
                quality,
                max_file_size_bytes,
            } => ImageConversionConfig::Enabled {
                quality,
                max_file_size_bytes,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MARKER: &str = "SYNTHETIC_MARKER";

    /// CU-7: what Swift can print of a descriptor or its file shows no path, name or alt text.
    #[test]
    fn descriptors_and_files_are_redacted() {
        let file = UploadFile::new(format!("/Users/{MARKER}/a.png"), 1, 2, 3, 4);
        let d = AttachmentDescriptor::new(
            file.clone(),
            format!("{MARKER}.png"),
            Some(MARKER.into()),
            true,
            "image/png".into(),
            None,
        );
        for text in [
            format!("{d:?}"),
            format!("{d}"),
            format!("{file:?}"),
            format!("{file}"),
        ] {
            assert!(!text.contains("SYNTHETIC"), "{text}");
        }
        assert!(d.filename().contains(MARKER) && d.file().path().contains(MARKER));
    }

    #[test]
    fn a_pending_upload_converts_field_for_field() {
        let d = core::AttachmentDescriptor {
            file: core::transport::FileIdentity {
                path: "/p/a.png".into(),
                device: 1,
                inode: 2,
                size: 30,
                mtime_ns: 4,
            },
            filename: "b.png".into(),
            description: Some("alt".into()),
            spoiler: true,
            content_type: "image/png".into(),
            original_md5: None,
        };
        let p = PendingUpload::from(core::PendingUpload {
            files: vec![core::PendingFile {
                descriptor: d,
                sent: 10,
            }],
            reply_to: Some(5),
            instant: true,
            state: core::UploadState::Failed(core::UploadFailure::HostRefused),
        });
        assert_eq!(
            (p.files[0].sent, p.files[0].size, p.reply_to, p.instant),
            (10, 30, Some(5), true)
        );
        assert_eq!(
            p.outcome,
            UploadOutcome::Failed {
                reason: UploadFailureReason::HostRefused
            }
        );
        let f = &p.files[0].descriptor;
        assert_eq!(
            (f.filename(), f.alt_text(), f.spoiler(), f.file().path()),
            ("b.png".into(), Some("alt".into()), true, "/p/a.png".into())
        );
    }
}
