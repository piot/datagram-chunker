use datagram_chunker::prelude::*;
use flood_rs::prelude::*;
use std::fmt;
use std::{
    fmt::{Display, Formatter},
    io,
};

#[derive(Debug, PartialEq)]
struct TestMessage {
    id: u32,
    content: String,
}

impl Serialize for TestMessage {
    fn serialize(&self, stream: &mut impl flood_rs::WriteOctetStream) -> io::Result<()> {
        stream.write_u32(self.id)?;
        let string_octets = &self.content.clone().into_bytes();
        stream.write_u16(string_octets.len() as u16)?;
        stream.write(string_octets)
    }
}

impl Deserialize for TestMessage {
    fn deserialize(stream: &mut impl flood_rs::ReadOctetStream) -> io::Result<Self> {
        let id = stream.read_u32()?;
        let length = stream.read_u16()? as usize;
        let mut buf = vec![0u8; length];
        stream.read(&mut buf)?;
        Ok(Self {
            id,
            content: String::from_utf8(buf)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "wrong"))?,
        })
    }
}

impl Display for TestMessage {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TestMessage {{ id: {}, content: {} }}",
            self.id, self.content
        )
    }
}

#[test]
fn test_serialize_to_datagrams_basic() {
    let messages = vec![
        TestMessage {
            id: 1,
            content: "Hello".into(),
        },
        TestMessage {
            id: 2,
            content: "World".into(),
        },
    ];
    let max_size = 1024;

    let result = serialize_to_datagrams(&messages, max_size);
    assert!(result.is_ok());

    let datagrams = result.unwrap();
    assert_eq!(datagrams.len(), 1); // Should fit in one datagram

    // Deserialize the datagram and verify the messages
    let deserialized_messages: Vec<TestMessage> = deserialize_datagram(&datagrams[0]).unwrap();
    assert_eq!(deserialized_messages, messages);
}

#[test]
fn test_serialize_to_datagrams_multiple_datagrams() {
    let messages = vec![
        TestMessage {
            id: 1,
            content: "A".repeat(500 - 6),
        }, // 500 octets
        TestMessage {
            id: 2,
            content: "B".repeat(500 - 6),
        }, // 500 octets
        TestMessage {
            id: 3,
            content: "C".repeat(500 - 6),
        }, // 500 octets
    ];
    let max_size = 1000;

    let result = serialize_to_datagrams(&messages, max_size);
    assert!(result.is_ok());

    let datagrams = result.unwrap();
    assert_eq!(datagrams.len(), 2); // Should require two datagrams

    // First datagram should contain messages 1 and 2
    let deserialized_messages1: Vec<TestMessage> = deserialize_datagram(&datagrams[0]).unwrap();
    assert_eq!(deserialized_messages1.len(), 2);
    assert_eq!(deserialized_messages1[0], messages[0]);
    assert_eq!(deserialized_messages1[1], messages[1]);

    // Second datagram should contain message 3
    let deserialized_messages2: Vec<TestMessage> = deserialize_datagram(&datagrams[1]).unwrap();
    assert_eq!(deserialized_messages2.len(), 1);
    assert_eq!(deserialized_messages2[0], messages[2]);
}

#[test]
fn test_serialize_to_datagrams_item_size_too_big() {
    let messages = vec![
        TestMessage {
            id: 1,
            content: "Short message".into(),
        },
        TestMessage {
            id: 2,
            content: "B".repeat(2000),
        }, // Exceeds max_size
    ];
    let max_size = 1024;

    let result = serialize_to_datagrams(&messages, max_size);
    assert!(result.is_err());

    match result.err().unwrap() {
        DatagramChunkerError::ItemSizeTooBig => (),
        other => panic!("Expected ItemSizeTooBig error, got {:?}", other),
    }
}

#[test]
fn test_deserialize_datagram_basic() {
    let messages = vec![
        TestMessage {
            id: 1,
            content: "Hello".into(),
        },
        TestMessage {
            id: 2,
            content: "World".into(),
        },
    ];

    // Serialize messages into a single datagram
    let serialized_datagram = {
        let mut chunker = DatagramChunker::new(1024);
        for msg in &messages {
            let mut temp = OutOctetStream::new();
            msg.serialize(&mut temp).unwrap();
            chunker.push(temp.octets_ref()).unwrap();
        }
        chunker.finalize().into_iter().next().unwrap()
    };

    // Deserialize the datagram
    let deserialized_messages: Vec<TestMessage> =
        deserialize_datagram(&serialized_datagram).unwrap();
    assert_eq!(deserialized_messages, messages);
}

#[test]
fn test_deserialize_datagram_malformed() {
    // Create a malformed datagram (incomplete serialization)
    let malformed_datagram = vec![0, 1, 2, 3, 4, 5]; // Arbitrary bytes

    let result: Result<Vec<TestMessage>, io::Error> = deserialize_datagram(&malformed_datagram);
    assert!(result.is_err());
}

#[test]
fn test_serialize_and_deserialize_empty_messages() {
    let messages: Vec<TestMessage> = vec![];
    let max_size = 1024;

    let serialized = serialize_to_datagrams(&messages, max_size).unwrap();
    assert_eq!(serialized.len(), 0);
}

#[test]
fn test_deserialize_empty_datagram() {
    let empty_datagram = vec![];
    let deserialized = deserialize_datagram::<TestMessage>(&empty_datagram);
    assert!(deserialized.is_ok());
    let messages = deserialized.unwrap();
    assert!(messages.is_empty());
}

#[test]
fn test_deserialize_multiple_messages_in_single_datagram() {
    let messages = vec![
        TestMessage {
            id: 1,
            content: "First message".into(),
        },
        TestMessage {
            id: 2,
            content: "Second message".into(),
        },
        TestMessage {
            id: 3,
            content: "Third message".into(),
        },
    ];

    // Serialize messages into a single datagram
    let serialized_datagram = {
        let mut chunker = DatagramChunker::new(2048);
        for msg in &messages {
            let mut temp = OutOctetStream::new();
            msg.serialize(&mut temp).unwrap();
            chunker.push(temp.octets_ref()).unwrap();
        }
        chunker.finalize().into_iter().next().unwrap()
    };

    // Deserialize the datagram
    let deserialized_messages: Vec<TestMessage> =
        deserialize_datagram(&serialized_datagram).unwrap();
    assert_eq!(deserialized_messages, messages);
}

#[test]
fn test_deserialize_datagrams_with_malformed_datagram() {
    let messages = vec![TestMessage {
        id: 1,
        content: "Hello".into(),
    }];

    // Serialize messages into a datagram
    let datagram1 = {
        let mut chunker = DatagramChunker::new(1024);
        for msg in &messages {
            let serialized = serialize_message(msg);
            chunker.push(&serialized).unwrap();
        }
        chunker.finalize().into_iter().next().unwrap()
    };

    // Create a malformed datagram (invalid UTF-8)
    let datagram2 = vec![0xff, 0xfe, 0xfd];

    let datagrams = vec![datagram1, datagram2];

    // Attempt to deserialize all datagrams, expecting an error
    let result = deserialize_datagrams::<TestMessage>(datagrams);
    assert!(result.is_err());
}

#[test]
fn test_deserialize_datagrams_all_malformed() {
    // Create multiple malformed datagrams (invalid UTF-8)
    let datagram1 = vec![0xff, 0xfe, 0xfd];
    let datagram2 = vec![0x80, 0x81, 0x82];
    let datagram3 = vec![0xc0, 0xc1, 0xc2];

    let datagrams = vec![datagram1, datagram2, datagram3];

    // Attempt to deserialize all datagrams, expecting an error
    let result = deserialize_datagrams::<TestMessage>(datagrams);
    assert!(result.is_err());
}

#[test]
fn test_deserialize_datagrams_with_partial_empty_datagrams() {
    let messages = vec![
        TestMessage {
            id: 1,
            content: "First".into(),
        },
        TestMessage {
            id: 2,
            content: "Second".into(),
        },
    ];

    // Serialize messages into a datagram
    let datagram1 = {
        let mut chunker = DatagramChunker::new(1024);
        for msg in &messages {
            let serialized = serialize_message(msg);
            chunker.push(&serialized).unwrap();
        }
        chunker.finalize().into_iter().next().unwrap()
    };

    // Create some empty datagrams
    let datagram2: Vec<u8> = vec![];
    let datagram3: Vec<u8> = vec![];

    let datagrams = vec![datagram1, datagram2, datagram3];

    // Deserialize all datagrams
    let deserialized_commands: Vec<TestMessage> = deserialize_datagrams(datagrams).unwrap();

    // Expected combined messages (empty datagrams contribute nothing)
    let expected = vec![
        TestMessage {
            id: 1,
            content: "First".into(),
        },
        TestMessage {
            id: 2,
            content: "Second".into(),
        },
    ];

    assert_eq!(deserialized_commands, expected);
}

/// Helper function to create a serialized byte vector from a TestMessage.
fn serialize_message(message: &TestMessage) -> Vec<u8> {
    let mut out_stream = OutOctetStream::new();
    message.serialize(&mut out_stream).unwrap();
    out_stream.octets_ref().to_vec()
}

#[test]
fn test_deserialize_datagrams_with_multiple_messages_per_datagram() {
    let messages1 = vec![
        TestMessage {
            id: 1,
            content: "Message1".into(),
        },
        TestMessage {
            id: 2,
            content: "Message2".into(),
        },
    ];

    let messages2 = vec![
        TestMessage {
            id: 3,
            content: "Message3".into(),
        },
        TestMessage {
            id: 4,
            content: "Message4".into(),
        },
    ];

    // Serialize multiple messages into a single datagram
    let datagram1 = {
        let mut chunker = DatagramChunker::new(2048);
        for msg in &messages1 {
            let serialized = serialize_message(msg);
            chunker.push(&serialized).unwrap();
        }
        chunker.finalize().into_iter().next().unwrap()
    };

    let datagram2 = {
        let mut chunker = DatagramChunker::new(2048);
        for msg in &messages2 {
            let serialized = serialize_message(msg);
            chunker.push(&serialized).unwrap();
        }
        chunker.finalize().into_iter().next().unwrap()
    };

    let datagrams = vec![datagram1, datagram2];

    // Deserialize all datagrams
    let deserialized_commands: Vec<TestMessage> = deserialize_datagrams(datagrams).unwrap();

    // Expected combined messages
    let expected = vec![
        TestMessage {
            id: 1,
            content: "Message1".into(),
        },
        TestMessage {
            id: 2,
            content: "Message2".into(),
        },
        TestMessage {
            id: 3,
            content: "Message3".into(),
        },
        TestMessage {
            id: 4,
            content: "Message4".into(),
        },
    ];

    assert_eq!(deserialized_commands, expected);
}
