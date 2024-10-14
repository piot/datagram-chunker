/*
 * Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/piot/datagram-chunker
 * Licensed under the MIT License. See LICENSE in the project root for license information.
 */

//! # Datagram Chunker Crate
//!
//! This crate provides functionality to serialize and deserialize messages into
//! datagrams with size constraints. It includes error handling mechanisms
//! and utilities to manage datagram chunking efficiently.
use err_rs::{ErrorLevel, ErrorLevelProvider};
use flood_rs::in_stream::InOctetStream;
use flood_rs::prelude::OutOctetStream;
use flood_rs::{Deserialize, ReadOctetStream, Serialize};
use std::fmt::{Debug, Display};
use std::{io, mem};

pub mod prelude;

/// Represents errors that can occur while chunking datagrams.
#[derive(Debug)]
pub enum DatagramChunkerError {
    /// The size of the item exceeds the maximum allowed datagram size.
    ItemSizeTooBig,
    /// An I/O error occurred.
    IoError(io::Error),
}

impl ErrorLevelProvider for DatagramChunkerError {
    fn error_level(&self) -> ErrorLevel {
        match self {
            DatagramChunkerError::ItemSizeTooBig => ErrorLevel::Critical,
            DatagramChunkerError::IoError(_) => ErrorLevel::Info,
        }
    }
}

/// A utility for chunking messages into datagrams with a specified maximum size.
pub struct DatagramChunker {
    datagrams: Vec<Vec<u8>>,
    current: Vec<u8>,
    max_size: usize,
}

impl DatagramChunker {
    /// Creates a new `DatagramChunker` with the given maximum datagram size.
    ///
    /// # Arguments
    ///
    /// * `max_size` - The maximum size of each datagram in bytes.
    pub fn new(max_size: usize) -> Self {
        Self {
            current: Vec::with_capacity(max_size),
            datagrams: Vec::new(),
            max_size,
        }
    }

    /// Pushes a message into the chunker, creating a new datagram if necessary.
    ///
    /// # Arguments
    ///
    /// * `data` - A byte slice to be added to the current datagram.
    ///
    /// # Errors
    ///
    /// Returns `DatagramChunkerError::ItemSizeTooBig` if the data size exceeds `max_size`.
    /// Propagates `DatagramChunkerError::IoError` if serialization fails.
    pub fn push(&mut self, buf: &[u8]) -> Result<(), DatagramChunkerError> {
        if buf.len() > self.max_size {
            return Err(DatagramChunkerError::ItemSizeTooBig);
        }

        if self.current.len() + buf.len() > self.max_size {
            self.datagrams.push(mem::take(&mut self.current));
            self.current = buf.to_vec();
        } else {
            self.current.extend_from_slice(buf);
        }

        Ok(())
    }

    pub fn finalize(mut self) -> Vec<Vec<u8>> {
        if !self.current.is_empty() {
            self.datagrams.push(self.current.clone());
        }
        self.datagrams
    }
}

impl From<io::Error> for DatagramChunkerError {
    fn from(value: io::Error) -> Self {
        Self::IoError(value)
    }
}

/// Serializes a collection of messages into datagrams, each not exceeding the specified maximum size.
///
/// # Type Parameters
///
/// * `I` - A reference to a slice of messages to serialize.
/// * `T` - The type of messages to serialize, which must implement `Serialize` and `Debug`.
///
/// # Arguments
///
/// * `messages` - The collection of messages to serialize.
/// * `max_datagram_size` - The maximum size of each datagram in bytes.
///
/// # Returns
///
/// A `Result` containing a vector of datagrams (`Vec<Vec<u8>>`) on success,
/// or a `DatagramChunkerError` on failure.
pub fn serialize_to_datagrams<I, T>(
    messages: I,
    max_datagram_size: usize,
) -> Result<Vec<Vec<u8>>, DatagramChunkerError>
where
    T: Serialize + Debug,
    I: AsRef<[T]>,
{
    let mut chunker = DatagramChunker::new(max_datagram_size);
    for message in messages.as_ref() {
        let mut temp = OutOctetStream::new();
        message.serialize(&mut temp)?;
        chunker.push(temp.octets_ref())?;
    }

    Ok(chunker.finalize())
}

/// Deserializes a single datagram into a vector of messages.
///
/// # Type Parameters
///
/// * `T` - The type of messages to deserialize, which must implement `Deserialize` and `Display`.
///
/// # Arguments
///
/// * `buf` - An octet slice representing the datagram to deserialize.
///
/// # Returns
///
/// A `Result` containing a vector of deserialized messages (`Vec<T>`) on success,
/// or an `io::Error` on failure.
///
pub fn deserialize_datagram<T>(buf: &[u8]) -> Result<Vec<T>, io::Error>
where
    T: Deserialize + Display,
{
    let mut messages = vec![];
    let mut in_stream = InOctetStream::new(buf);

    while !&in_stream.has_reached_end() {
        messages.push(T::deserialize(&mut in_stream)?);
    }

    Ok(messages)
}

/// Deserializes multiple datagrams into a unified vector of messages.
///
/// This function processes a collection of datagrams (`Vec<Vec<u8>>`), deserializing each one
/// into its respective vector of messages and aggregating all messages into a single vector.
///
/// # Type Parameters
///
/// * `T` - The type of message to deserialize, which must implement `Deserialize` and `Display`.
///
/// # Arguments
///
/// * `datagrams` - A vector of datagrams, where each datagram is represented as a `Vec<u8>`.
///
/// # Returns
///
/// A `Result` containing a unified vector of deserialized commands (`Vec<CommandT>`) on success,
/// or a `io::Error` if any deserialization fails.
pub fn deserialize_datagrams<T>(datagrams: Vec<Vec<u8>>) -> Result<Vec<T>, io::Error>
where
    T: Deserialize + Display,
{
    let mut all_messages = Vec::new();
    for datagram in datagrams {
        let messages = deserialize_datagram(&datagram)?;
        all_messages.extend(messages);
    }
    Ok(all_messages)
}
