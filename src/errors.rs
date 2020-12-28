use async_std::channel::{RecvError, SendError};
use std::fmt::Debug;
use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CrablerError {
    #[error("io error")]
    Io(#[from] io::Error),

    #[error("reqwest error")]
    Reqwest(#[from] reqwest::Error),

    #[error("failed to recieve workload from async channel")]
    AsyncRecvError(#[from] RecvError),

    #[error("failed to send workload to async channel")]
    AsyncSendError(String),
}

impl<T: Debug> From<SendError<T>> for CrablerError {
    fn from(err: SendError<T>) -> Self {
        Self::AsyncSendError(format!(
            "Failed to send payload {:?} to async channel",
            err.into_inner()
        ))
    }
}

pub type Result<T> = std::result::Result<T, CrablerError>;
