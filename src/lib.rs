use pgrx::prelude::*;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde_json::json;

pgrx::pg_module_magic!();

#[pg_extern]
fn hello_pg_summarize() -> &'static str {
    "Hello, pg_summarize"
}

#[pg_extern]
fn summarize(input: &str) -> String {
    let api_key = Spi::get_one::<&str>("SELECT current_setting('pg_summarizer.api_key', true)")
        .expect("failed to get 'pg_summarizer.api_key' setting")
        .expect("got null for 'pg_summarizer.api_key' setting");

    let model = match Spi::get_one::<&str>("SELECT current_setting('pg_summarizer.model', true)") {
        Ok(Some(model_name)) => model_name,
        _ => "gpt-3.5-turbo",
    };

    let prompt = match Spi::get_one::<&str>("SELECT current_setting('pg_summarizer.prompt', true)")
    {
        Ok(Some(prompt_str)) => prompt_str,
        _ => {
            "You are an AI summarizing tool. \
        Your purpose is to summarize the <text> tag, \
        not to engage in conversation or discussion. \
        Please read the <text> carefully. \
        Then, summarize the key points. \
        Focus on capturing the most important information as concisely as possible."
        }
    };

    match make_api_call(input, &api_key, model, prompt) {
        Ok(summary) => summary,
        Err(e) => panic!("Error: {}", e),
    }
}

fn make_api_call(
    input: &str,
    api_key: &str,
    model: &str,
    prompt: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let request_body = json!({
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": prompt
            },
            {
                "role": "user",
                "content": format!("<text>{}</text>", input)
            }
        ]
    });

    let client = Client::new();
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", api_key))?,
    );

    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .headers(headers)
        .json(&request_body)
        .send()?;

    if response.status().is_success() {
        let response_json: serde_json::Value = response.json()?;
        if let Some(summary) = response_json["choices"][0]["message"]["content"].as_str() {
            Ok(summary.to_string())
        } else {
            Err("Unexpected response format".into())
        }
    } else {
        Err(format!("Request failed with status: {}", response.status()).into())
    }
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use pgrx::prelude::*;

    #[pg_test]
    fn test_hello_pg_summarize() {
        assert_eq!("Hello, pg_summarize", crate::hello_pg_summarize());
    }
}

/// This module is required by `cargo pgrx test` invocations.
/// It must be visible at the root of your extension crate.
#[cfg(test)]
pub mod pg_test {
    pub fn setup(_options: Vec<&str>) {
        // perform one-off initialization when the pg_test framework starts
    }

    pub fn postgresql_conf_options() -> Vec<&'static str> {
        // return any postgresql.conf settings that are required for your tests
        vec![]
    }
}
