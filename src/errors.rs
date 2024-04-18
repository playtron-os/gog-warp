use std::fmt::{Debug, Display, Formatter};

pub(crate) type EmptyResult = Result<(), Error>;
type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug)]
pub enum ErrorKind {
    NotLoggedIn,
    Unauthorized,
    Json,
    Request,
}

pub struct Error {
    kind: ErrorKind,
    source: Option<BoxError>,
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
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            ErrorKind::Json => f.write_str("json serialization error"),
            ErrorKind::NotLoggedIn => f.write_str("not logged-in error"),
            ErrorKind::Unauthorized => f.write_str("token is no longer valid"),
            ErrorKind::Request => f.write_str("network request error"),
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

pub(crate) fn json_error<E: Into<BoxError>>(err: E) -> Error {
    Error::new(ErrorKind::Json, Some(err))
}

pub(crate) fn request_error<E: Into<BoxError>>(err: E) -> Error {
    Error::new(ErrorKind::Request, Some(err))
}
