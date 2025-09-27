use crate::data::{DocumentRef, KeywordRef};
use worker::*;

#[durable_object]
pub struct Journal {
    entries: [JournalCommand; 2048],
}

#[derive(Clone, Copy)]
#[repr(u8)]
enum JournalCommand {
    None,
    KeywordUpdate(KeywordEntryUpdate),
}

#[derive(Clone, Copy)]
struct KeywordEntryUpdate {
    kv: KeywordRef,
    doc_kv: DocumentRef,
    action: JournalAction,
}

#[derive(Clone, Copy)]
enum JournalAction {
    Append,
    Delete,
}

impl DurableObject for Journal {
    fn new(state: State, _: Env) -> Self {
        Self {
            entries: [JournalCommand::None; 2048],
        }
    }

    async fn fetch(&self, req: Request) -> worker::Result<worker::Response> {
        let value = state.storage().get("key").unwrap_or(Some(0)).unwrap_or(0);
        match req.method() {
            Method::Post => {
                // Handle POST request
                Ok(worker::Response::ok("Janitor DO POST response"))
            }
            _ => Ok(worker::Response::error("Method Not Allowed", 405)?),
        }
    }

    async fn alarm(&self) -> Result<Response> {
        // Handle alarm event
        Ok(worker::Response::ok("Alarm triggered"))
    }
}
