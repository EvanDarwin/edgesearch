use core::panic;

use crate::durable::journal_data::{JournalCommand, JournalData, JournalUpdateEntry};
use serde::{Deserialize, Serialize};
use worker::*;

#[durable_object]
pub struct Journal {
    state: State,
    journal: JournalData,
    sql: SqlStorage,
}

static TABLE_NAME_JOURNAL_DATA: &str = "journal_data";
static DATA_KEY_JOURNALDATA: &str = "journal";

impl Journal {
    fn get_journal(sql: &SqlStorage) -> JournalData {
        #[derive(Deserialize)]
        struct JournalDataRow {
            value: String,
        }
        // Select the data if it exists
        let result: Vec<JournalDataRow> = sql
            .exec("SELECT * FROM ?", vec![TABLE_NAME_JOURNAL_DATA.into()])
            .unwrap()
            .to_array::<JournalDataRow>()
            .unwrap();

        if result.len() == 0 {
            panic!("No journal data found");
        }

        // Get the string value in the first row and first column
        let journal_raw_data = &result[0].value;
        let journal = serde_json::from_str::<JournalData>(journal_raw_data).unwrap();

        journal
    }

    fn save_journal(&mut self) -> worker::Result<()> {
        self.sql
            .exec(
                "INSERT OR REPLACE INTO ?(value) VALUES (?) WHERE key='?';",
                vec![
                    TABLE_NAME_JOURNAL_DATA.into(),
                    serde_json::to_string(&self.journal)
                        .unwrap()
                        .to_string()
                        .as_str()
                        .into(),
                    DATA_KEY_JOURNALDATA.into(),
                ],
            )
            .unwrap();
        Ok(())
    }

    fn create_table(sql: &SqlStorage) {
        sql.exec(
            &format!(
                "CREATE TABLE IF NOT EXISTS {} (key TEXT PRIMARY KEY, value TEXT);",
                TABLE_NAME_JOURNAL_DATA
            ),
            vec![],
        )
        .unwrap();
    }

    // Return the current time to maximum precision
    pub fn now() -> u64 {
        worker::Date::now().as_millis()
    }
}

impl DurableObject for Journal {
    fn new(state: State, _: Env) -> Self {
        let sql = state.storage().sql();

        Self::create_table(&sql);
        let journal = Self::get_journal(&sql);

        Self {
            sql,
            state,
            journal,
        }
    }

    async fn fetch(&self, req: Request) -> worker::Result<worker::Response> {
        let doFullIndex = self.journal.last_index < (Self::now() - 1000);

        match req.method() {
            Method::Post => {
                req.json::<Vec>().await.unwrap();

                // Handle POST request
                worker::Response::ok(r#"{"ok":true}"#)
            }
            _ => worker::Response::error("Method Not Allowed", 405),
        }
    }

    async fn alarm(&self) -> Result<Response> {
        // Handle alarm event
        worker::Response::ok("Alarm triggered")
    }
}
