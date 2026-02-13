//! Cortex API protocol domain modules.
//!
//! This namespace groups wire-compatible JSON-RPC protocol structures by domain:
//! - [`rpc`]: JSON-RPC request/response/error envelope types.
//! - [`constants`]: method names, error codes, and stream constants.
//! - [`headset`]: headset discovery and config-mapping payloads.
//! - [`session`]: session lifecycle payloads.
//! - [`streams`]: raw stream events and parsed stream data structures.
//! - [`records`]: record and marker payloads.
//! - [`profiles`]: profile query and setup payloads.
//! - [`training`]: detection/training and advanced BCI payloads.
//! - [`auth`]: authentication/user-login payloads.
//! - [`subjects`]: subject/demographic payloads.

pub mod auth;
pub mod constants;
pub mod headset;
pub mod profiles;
pub mod records;
pub mod rpc;
pub mod session;
pub mod streams;
pub mod subjects;
pub mod training;
