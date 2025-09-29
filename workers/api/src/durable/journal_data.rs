use serde::{Deserialize, Serialize};

use crate::{
    data::{DocumentRef, KeywordRef},
    durable::journal::Journal,
};

#[derive(Serialize, Deserialize)]
#[repr(u8)]
pub enum JournalCommand {
    None,
    DocumentUpdate,
    KeywordShardUpdate,
}

#[derive(Serialize, Deserialize)]
#[repr(u8)]
pub enum JournalAction {
    Append,
    Delete,
}

#[derive(Serialize, Deserialize)]
pub struct JournalUpdateEntry {
    #[serde(rename = "k")]
    pub kind: JournalCommand,
    #[serde(rename = "w")]
    pub kv: KeywordRef,
    #[serde(rename = "d")]
    pub doc_kv: DocumentRef,
    #[serde(rename = "a")]
    pub action: JournalAction,
}

#[derive(Serialize, Deserialize)]
pub struct JournalData {
    pub entries: Vec<JournalUpdateEntry>,
    pub last_index: u64,
}

impl JournalData {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            last_index: Journal::now(),
        }
    }
}
