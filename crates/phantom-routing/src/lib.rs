//! PHANTOM Routing Engine
//! 
//! Oblivious packet forwarding using FHE
//!
//! This crate implements the core routing logic that enables PHANTOM's
//! oblivious routing. Nodes process packets using FHE without learning
//! the routing path or metadata.

pub mod forwarder;
pub mod peer_selector;
pub mod path_builder;
pub mod path_validator;
pub mod wire_format;
pub mod forwarding_protocol;
pub mod sparse_table;

pub use forwarder::{ObliviousForwarder, RoutingDecision, DropReason, ForwardingStats};
pub use peer_selector::{PeerSelector, NodeInfo, NodeCapabilities, SelectionStrategy};
pub use path_builder::{PathBuilder, PhantomPath};
pub use path_validator::{PathValidator, ValidationResult, QualityMetrics, QualityWeights};
pub use wire_format::{WireHeader, serialize_packet, deserialize_packet, WireError, PROTOCOL_VERSION};
pub use forwarding_protocol::{NetworkSimulator, PathTrace, PathStatus, HopResult};
pub use sparse_table::{SparseRoutingTable, SparseRoutingEntry, SparseRoutingLookup};
