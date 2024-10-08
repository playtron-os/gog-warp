use std::fmt::{Debug, Display, Formatter};

pub(crate) type EmptyResult = Result<(), Error>;
type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, Clone)]
pub enum ErrorKind {
    NotLoggedIn,
    Unauthorized,
    InvalidSession,
    Cancelled,
    Serde,
    Request,
    Task,
    Io,
    Zlib,
    Xdelta(String),
    MaximumRetries,
    #[cfg(feature = "downloader")]
    DownloaderBuilder,
    NotReady,
}

pub struct Error {
    kind: ErrorKind,
    pub source: Option<BoxError>,
}

impl Error {
    pub fn new<E>(kind: ErrorKind, err: Option<E>) -> Self
    where
        E: Into<BoxError>,
    {
        Self {
            kind,
            source: err.map(Into::into),
        }
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind.clone()
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            ErrorKind::Serde => f.write_str("serialization error"),
            ErrorKind::NotLoggedIn => f.write_str("not logged-in error"),
            ErrorKind::Unauthorized => f.write_str("token is no longer valid"),
            ErrorKind::InvalidSession => {
                f.write_str("current refresh token is no longer valid, re-login is required")
            }
            ErrorKind::Request => f.write_str("network request error"),
            ErrorKind::Io => f.write_str("io error"),
            ErrorKind::MaximumRetries => f.write_str("maximum retries exceeded"),
            ErrorKind::Zlib => f.write_str("zlib error"),
            ErrorKind::Xdelta(msg) => f.write_fmt(format_args!("decompression error {}", msg)),
            ErrorKind::Cancelled => f.write_str("operation was cancelled"),
            ErrorKind::Task => f.write_str("error occured in the task executor"),
            #[cfg(feature = "downloader")]
            ErrorKind::DownloaderBuilder => f.write_str("builder error, required field missing"),
            ErrorKind::NotReady => f.write_str("preconditions weren't met"),
        }
    }
}

impl Debug for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut builder = f.debug_struct("gog_warp::Error");
        builder.field("kind", &self.kind);
        if let Some(source) = &self.source {
            builder.field("source", source);
        }
        builder.finish()
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_ref().map(|err| &**err as _)
    }
}

pub(crate) fn not_logged_in_error() -> Error {
    Error::new(ErrorKind::NotLoggedIn, None::<BoxError>)
}

pub(crate) fn unauthorized_error() -> Error {
    Error::new(ErrorKind::Unauthorized, None::<BoxError>)
}

pub(crate) fn invalid_session_error() -> Error {
    Error::new(ErrorKind::InvalidSession, None::<BoxError>)
}

pub(crate) fn cancelled_error() -> Error {
    Error::new(ErrorKind::Cancelled, None::<BoxError>)
}

pub(crate) fn not_ready_error(err: &str) -> Error {
    Error::new(ErrorKind::NotReady, Some(err))
}

#[cfg(feature = "downloader")]
pub(crate) fn dbuilder_error() -> Error {
    Error::new(ErrorKind::DownloaderBuilder, None::<BoxError>)
}

pub(crate) fn maximum_retries_error() -> Error {
    Error::new(ErrorKind::MaximumRetries, None::<BoxError>)
}

pub(crate) fn serde_error<E: Into<BoxError>>(err: E) -> Error {
    Error::new(ErrorKind::Serde, Some(err))
}

pub(crate) fn request_error<E: Into<BoxError>>(err: E) -> Error {
    Error::new(ErrorKind::Request, Some(err))
}

pub(crate) fn zlib_error<E: Into<BoxError>>(err: E) -> Error {
    Error::new(ErrorKind::Zlib, Some(err))
}

pub(crate) fn xdelta_error(msg: String) -> Error {
    Error::new(ErrorKind::Xdelta(msg), None::<BoxError>)
}

pub(crate) fn io_error<E: Into<BoxError>>(err: E) -> Error {
    Error::new(ErrorKind::Io, Some(err))
}

pub(crate) fn task_error<E: Into<BoxError>>(err: E) -> Error {
    Error::new(ErrorKind::Task, Some(err))
}
