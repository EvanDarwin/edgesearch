use url::form_urlencoded;

pub fn url_decode(input: &str) -> String {
    form_urlencoded::parse(input.as_bytes())
        .map(|(k, _)| k.into_owned())
        .collect()
}
