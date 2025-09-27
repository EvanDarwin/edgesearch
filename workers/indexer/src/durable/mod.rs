//! This module includes the durable object implementations, particularly for
//! the Compare-and-Swap (CAS) write production in a globally distributed system.

use worker::{DurableObject, Method, Request};

mod journal;
