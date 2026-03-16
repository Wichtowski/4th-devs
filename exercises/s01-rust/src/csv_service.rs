use anyhow::{Context, Result};
use csv::{ReaderBuilder, StringRecord};
use serde::de::DeserializeOwned;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct CsvService;

impl CsvService {
    pub fn read_records<T>(csv_text: &str) -> Result<Vec<T>>
    where
        T: DeserializeOwned,
    {
        let mut reader = ReaderBuilder::new()
            .flexible(true)
            .from_reader(csv_text.as_bytes());
        let headers = reader.headers().context("Failed to read CSV headers")?.clone();
        let mut records = Vec::new();

        for record in reader.records() {
            let record = record.context("Failed to parse CSV record")?;
            let value = Self::record_to_json(&headers, &record);
            let parsed = serde_json::from_value(value).context("Invalid CSV record")?;
            records.push(parsed);
        }

        Ok(records)
    }

    fn record_to_json(headers: &StringRecord, record: &StringRecord) -> Value {
        let object = headers
            .iter()
            .zip(record.iter())
            .map(|(header, value)| (header.to_owned(), Value::String(value.to_owned())))
            .collect::<serde_json::Map<String, Value>>();
        Value::Object(object)
    }
}
