//! The first real user: a stored event page's wire crossing, from the
//! envelope an old store holds (payload 3, meta 8, sequence 9) to the
//! current one (event 1, meta 2, sequence 3), as `forward[0]` of a registry
//! whose versions are the two packages' prost messages. The frozen v0 copy is
//! spelled here field for field as the two packages have them.

use super::{encode, load, Decoders};
use crate::{MigrateError, Registry};

mod v0 {
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct Uuid {
        #[prost(bytes = "vec", tag = "1")]
        pub value: Vec<u8>,
    }
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct FactOrigin {
        #[prost(string, tag = "1")]
        pub external_id: String,
    }
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct Any {
        #[prost(string, tag = "1")]
        pub type_url: String,
        #[prost(bytes = "vec", tag = "2")]
        pub value: Vec<u8>,
    }
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct EventMeta {
        #[prost(message, optional, tag = "17")]
        pub command_id: Option<Uuid>,
        #[prost(message, optional, tag = "16")]
        pub fact: Option<FactOrigin>,
        #[prost(message, optional, tag = "14")]
        pub correlation_id: Option<Uuid>,
        #[prost(string, tag = "7")]
        pub rules_version: String,
    }
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct EventPage {
        #[prost(message, optional, tag = "3")]
        pub payload: Option<Any>,
        #[prost(message, optional, tag = "8")]
        pub meta: Option<EventMeta>,
        #[prost(uint64, tag = "9")]
        pub sequence: u64,
    }
}

mod v1 {
    pub use super::v0::{Any, FactOrigin, Uuid};

    #[derive(Clone, PartialEq, prost::Message)]
    pub struct CommandCause {
        #[prost(message, optional, tag = "1")]
        pub command_id: Option<Uuid>,
    }
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct FactCause {
        #[prost(message, optional, tag = "1")]
        pub command_id: Option<Uuid>,
        #[prost(message, optional, tag = "2")]
        pub origin: Option<FactOrigin>,
    }
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct EventMeta {
        #[prost(oneof = "event_meta::Causation", tags = "1, 2")]
        pub causation: Option<event_meta::Causation>,
        #[prost(message, optional, tag = "3")]
        pub correlation_id: Option<Uuid>,
        #[prost(string, tag = "5")]
        pub rules_version: String,
    }
    pub mod event_meta {
        #[derive(Clone, PartialEq, prost::Oneof)]
        pub enum Causation {
            #[prost(message, tag = "1")]
            Command(super::CommandCause),
            #[prost(message, tag = "2")]
            Fact(super::FactCause),
        }
    }
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct EventPage {
        #[prost(message, optional, tag = "1")]
        pub event: Option<Any>,
        #[prost(message, optional, tag = "2")]
        pub meta: Option<EventMeta>,
        #[prost(uint64, tag = "3")]
        pub sequence: u64,
    }
}

impl From<v0::EventMeta> for v1::EventMeta {
    fn from(old: v0::EventMeta) -> v1::EventMeta {
        use v1::event_meta::Causation;
        let causation = match (old.fact, old.command_id) {
            (Some(origin), command_id) => Some(Causation::Fact(v1::FactCause {
                command_id,
                origin: Some(origin),
            })),
            (None, Some(id)) => Some(Causation::Command(v1::CommandCause {
                command_id: Some(id),
            })),
            (None, None) => None,
        };
        v1::EventMeta {
            causation,
            correlation_id: old.correlation_id,
            rules_version: old.rules_version,
        }
    }
}

impl From<v0::EventPage> for v1::EventPage {
    fn from(old: v0::EventPage) -> v1::EventPage {
        v1::EventPage {
            event: old.payload,
            meta: old.meta.map(Into::into),
            sequence: old.sequence,
        }
    }
}

/// The wire crossing is `forward[0]`.
fn registry() -> Registry<v1::EventPage> {
    Registry::builder(0, 1)
        .forward::<v0::EventPage, v1::EventPage>(0)
        .build()
        .unwrap()
}

fn decoders() -> Decoders {
    Decoders::new()
        .at::<v0::EventPage>(0)
        .at::<v1::EventPage>(1)
}

fn uuid(byte: u8) -> v0::Uuid {
    v0::Uuid {
        value: vec![byte; 16],
    }
}

fn any() -> v0::Any {
    v0::Any {
        type_url: "type.example/Thing".into(),
        value: vec![1, 2, 3],
    }
}

#[test]
fn a_command_caused_page_crosses_as_forward_zero() {
    use prost::Message;
    let stored = v0::EventPage {
        payload: Some(any()),
        meta: Some(v0::EventMeta {
            command_id: Some(uuid(1)),
            fact: None,
            correlation_id: Some(uuid(2)),
            rules_version: "1.0".into(),
        }),
        sequence: 41,
    }
    .encode_to_vec();
    let crossed = load(&registry(), &decoders(), &stored, 0).unwrap();
    assert_eq!(crossed.applied, [0]);
    assert_eq!((crossed.from, crossed.to), (0, 1));
    let expected = v1::EventPage {
        event: Some(any()),
        meta: Some(v1::EventMeta {
            causation: Some(v1::event_meta::Causation::Command(v1::CommandCause {
                command_id: Some(uuid(1)),
            })),
            correlation_id: Some(uuid(2)),
            rules_version: "1.0".into(),
        }),
        sequence: 41,
    };
    assert_eq!(crossed.object, expected);
    // Every write is current: the bytes a store then keeps are the new
    // package's, field 1 the event, 2 the meta and 3 the sequence.
    assert_eq!(encode(&crossed.object), expected.encode_to_vec());
    assert_eq!(encode(&crossed.object)[0], 0x0a);
}

#[test]
fn a_fact_caused_page_keeps_its_command_id_and_origin() {
    use prost::Message;
    let stored = v0::EventPage {
        payload: Some(any()),
        meta: Some(v0::EventMeta {
            command_id: Some(uuid(1)),
            fact: Some(v0::FactOrigin {
                external_id: "ext-1".into(),
            }),
            ..Default::default()
        }),
        sequence: 2,
    }
    .encode_to_vec();
    let crossed = load(&registry(), &decoders(), &stored, 0).unwrap().object;
    let Some(v1::event_meta::Causation::Fact(fact)) = crossed.meta.unwrap().causation else {
        panic!("a fact's cause")
    };
    assert_eq!(fact.command_id, Some(uuid(1)));
    assert_eq!(fact.origin.unwrap().external_id, "ext-1");
}

#[test]
fn a_page_with_no_meta_and_sequence_zero_crosses() {
    use prost::Message;
    let stored = v0::EventPage {
        payload: Some(any()),
        meta: None,
        sequence: 0,
    }
    .encode_to_vec();
    let crossed = load(&registry(), &decoders(), &stored, 0).unwrap().object;
    assert_eq!(
        crossed,
        v1::EventPage {
            event: Some(any()),
            meta: None,
            sequence: 0
        }
    );
}

#[test]
fn a_current_page_is_loaded_as_it_is() {
    use prost::Message;
    let page = v1::EventPage {
        event: Some(any()),
        meta: None,
        sequence: 5,
    };
    let m = load(&registry(), &decoders(), &page.encode_to_vec(), 1).unwrap();
    assert!(!m.changed());
    assert_eq!(m.object, page);
}

#[test]
fn bytes_that_are_not_the_versions_message_are_told() {
    // A current page's bytes, read as version 0: field 3 is a varint there.
    use prost::Message;
    let page = v1::EventPage {
        event: None,
        meta: None,
        sequence: 5,
    }
    .encode_to_vec();
    let e = load(&registry(), &decoders(), &page, 0).unwrap_err();
    assert!(matches!(e, MigrateError::Undecodable { version: 0, .. }));
}

#[test]
fn a_version_with_no_decoder_or_outside_the_registry_is_refused() {
    let none = Decoders::new().at::<v1::EventPage>(1);
    assert_eq!(
        load(&registry(), &none, &[], 0).unwrap_err(),
        MigrateError::NoLoader { version: 0 }
    );
    assert_eq!(
        load(&registry(), &decoders(), &[], 2).unwrap_err(),
        MigrateError::TooNew {
            found: 2,
            current: 1
        }
    );
}
