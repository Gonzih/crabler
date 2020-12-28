use async_std::channel::{RecvError, SendError};
use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CrablerError {
    #[error("io error")]
    Io(#[from] io::Error),

    #[error("reqwest error")]
    Reqwest(#[from] reqwest::Error),

    #[error("failed to recieve workload from async channel")]
    AsyncRecvError(#[from] async_std::channel::RecvError),

    #[error("failed to send workload to async channel")]
    AsyncSendError(String),
}

impl<T> From<SendError<T>> for CrablerError {
    fn from(err: SendError<T>) -> Self {
        Self::AsyncSendError("failed at it".to_string())
    }
}

pub type Result<T> = std::result::Result<T, CrablerError>;
