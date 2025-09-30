use serde::de::Visitor;

pub struct LengthPrefixed {
    pub bytes: Vec<u8>,
}

impl LengthPrefixed {}

impl ToString for LengthPrefixed {
    fn to_string(&self) -> String {
        std::str::from_utf8(&self.bytes).unwrap().to_string()
    }
}

impl Into<Vec<u8>> for LengthPrefixed {
    fn into(self) -> Vec<u8> {
        self.bytes
    }
}

impl AsRef<[u8]> for LengthPrefixed {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

impl<'de> Visitor<'de> for LengthPrefixed {
    type Value = LengthPrefixed;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a length-prefixed byte array")
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if v.len() < 4 {
            return Err(E::custom("buffer too short for length prefix"));
        }

        // first 4 bytes are length (u32, little endian)
        let len = u32::from_le_bytes([v[0], v[1], v[2], v[3]]) as usize;
        if v.len() < 4 + len {
            return Err(E::custom("buffer shorter than length prefix"));
        }
        Ok(LengthPrefixed {
            bytes: v[4..4 + len].to_vec(),
        })
    }
}

// Read the first 4 bytes as u8, convert to a u32 size of n, then
// read the next n bytes as the data. Then repeat until we run out of data
pub fn read_length_prefixed<'se, T: serde::Deserialize<'se>>(data: &'se Vec<u8>) -> Vec<T> {
    let mut pos = 0u32;
    let mut results: Vec<T> = Vec::new();

    while pos < data.len() as u32 {
        let lp = read_one_length_prefixed(&data[pos as usize..]).unwrap_or(&[]);
        let obj = serde_json::from_slice::<T>(lp).unwrap();
        pos += 4 + lp.len() as u32;
        results.push(obj);
    }

    results
}

fn read_one_length_prefixed(data: &[u8]) -> Option<&[u8]> {
    if data.len() < 4 {
        return None;
    }
    let size = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    if data.len() < 4 + size {
        return None;
    }
    Some(&data[4..4 + size])
}
