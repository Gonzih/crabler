use async_std::channel::{RecvError, SendError};
use std::fmt::Debug;
use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CrablerError {
    #[error("io error {0}")]
    Io(#[from] io::Error),

    #[error("failed to recieve workload from async channel {0}")]
    AsyncRecvError(#[from] RecvError),

    #[error("failed to send workload to async channel: {0}")]
    AsyncSendError(String),

    #[error("surf error {0}: {1}")]
    SurfError(surf::StatusCode, String),
}

impl<T: Debug> From<SendError<T>> for CrablerError {
    fn from(err: SendError<T>) -> Self {
        Self::AsyncSendError(format!(
            "{:?}",
            err.into_inner()
        ))
    }
}

impl From<surf::Error> for CrablerError {
    fn from(err: surf::Error) -> Self {
        Self::SurfError(err.status(), format!("{:?}", err))
    }
}

pub type Result<T> = std::result::Result<T, CrablerError>;
