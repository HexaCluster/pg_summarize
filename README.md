# Extending PostgreSQL with Rust: Building an Extension with OpenAI Integration

PostgreSQL is a powerful and versatile database management system. One of its strengths lies in its extensibility. In this blog, we will explore how to extend PostgreSQL using Rust, specifically focusing on creating a custom extension called `pg_summarize` that integrates with the OpenAI API. This extension will include a basic "Hello, pg_summarize!" function and another function to summarize text using OpenAI's models.

## Introduction to PostgreSQL Extensions

PostgreSQL extensions are packages that add functionality to the database, allowing you to introduce new types, functions, and operators. They enable you to tailor PostgreSQL to specific use cases, enhancing its capabilities without modifying the core system.

## Why Rust?

Rust is a systems programming language known for its performance and safety. When combined with PostgreSQL, it provides a powerful platform for creating high-performance database extensions. We'll use the `pgrx` crate, which simplifies writing PostgreSQL extensions in Rust.

## Setting Up the Environment

First, ensure you have Rust installed on your system. If not, install it using [rustup.rs](https://rustup.rs).

Next, add the `pgrx` crate to your project:

```sh
cargo install --locked cargo-pgrx
```

Initialize the "PGRX Home" directory:

```sh
cargo pgrx init
```

## Initializing the Project

Create the initial Rust project directory to build the `pgrx` extension:

```sh
cargo pgrx new pg_summarize
cd pg_summarize
```

This command creates the following project structure:

```
.
├── Cargo.toml
├── pg_summarize.control
├── sql
└── src
    └── lib.rs
```

You should already see the `hello_pg_summarize` function in `src/lib.rs`:

```rust
...
#[pg_extern]
fn hello_pg_summarize() -> &'static str {
    "Hello, pg_summarize"
}
...
```

Compile and Run the extension:

```sh
cargo pgrx run
```

This command compiles the extension to a shared library, copies it to the specified Postgres installation, starts that Postgres instance, and connects you to a database named the same as the extension. Load the extension and call the function.

In the PostgreSQL shell:

```sql
CREATE EXTENSION pg_summarize;
SELECT hello_pg_summarize();
```

The output should be:

```
 hello_pg_summarize
----------------------
 Hello, pg_summarize
(1 row)
```

Voila! You’ve built a PostgreSQL extension using Rust.

## Extending the Extension: Implementing a Summarize Function with OpenAI Integration

Let's create a function that uses the OpenAI API to summarize text. This function will retrieve the API key and other settings from PostgreSQL, make a call to the OpenAI API, and return the summary.

### Installing Dependencies

To call the OpenAI endpoint, you need to make a POST request. For this, you'll use `reqwest` and `serde_json` to handle JSON responses. Install them with:

```sh
cargo add reqwest --features json,blocking
cargo add serde_json
```

### Creating the Core Functionality

Add the following function to make the API call:

```rust
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE, AUTHORIZATION};
use serde_json::json;

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
```

- **Constructing the Request Body**: The function `make_api_call` builds a JSON request body containing the model and a series of messages with roles "system" and "user". The user's message includes the input text wrapped in `<text>` tags.
- **Setting Up the HTTP Client and Headers**: An HTTP client is created using `Client::new()`. Headers for content type and authorization are set up, with the authorization header using the provided API key.
- **Sending the POST Request**: The function sends a POST request to the OpenAI API endpoint for chat completions, including the constructed headers and JSON request body. It captures the response from the API.
- **Handling the Response**: If the response status indicates success, the function extracts and returns the summarized content from the response JSON. If the response format is unexpected or the request fails, it returns an error.

### Wrapping and Exposing the Function to PostgreSQL

Now, let’s wrap the core function `make_api_call` with a function `summarize` and expose it to PostgreSQL using the `#[pg_extern]` macro provided by `pgrx`:

```rust
#[pg_extern]
fn summarize(input: &str) -> String {
    let api_key = Spi::get_one::<&str>("SELECT current_setting('pg_summarizer.api_key', true)")
        .expect("failed to get 'pg_summarizer.api_key' setting")
        .expect("got null for 'pg_summarizer.api_key' setting");

    let model = match Spi::get_one::<&str>("SELECT current_setting('pg_summarizer.model', true)") {
        Ok(Some(model_name)) => model_name,
        _ => "gpt-3.5-turbo",
    };

    let prompt = match Spi::get_one::<&str>("SELECT current_setting('pg_summarizer.prompt', true)") {
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
```

- **Configuration Retrieval**: The `summarize` function, marked with `#[pg_extern]` to expose it to PostgreSQL, retrieves configuration settings (API key, model, and prompt) from the PostgreSQL database using `Spi::get_one`. Defaults are used if settings are not found.
- **Constructing the Prompt**: A default prompt is defined for the summarization task, focusing on extracting key points from the `<text>` tag content. This prompt is used if no custom prompt is retrieved from the database.
- **Making the API Call and Handling the Result**: The function calls `make_api_call` with the input text, API key, model, and prompt. If the API call is successful, the summary is returned. If an error occurs, the function panics with an error message.

### Running and Testing the Extension

To make the extension configurable, set PostgreSQL settings for the API key, model, and prompt. These settings can be added to your PostgreSQL configuration or set at runtime.

```sql
-- Set the OpenAI API key
ALTER SYSTEM SET pg_summarizer.api_key = 'your_openai_api_key';

-- Optionally set the model at SYSTEM level
ALTER SYSTEM SET pg_summarizer.model = 'gpt-3.5-turbo';

-- Or, optionally set the prompt at SESSION level
SET pg_summarizer.prompt = 'Your custom prompt here';

-- Reload the configuration if set at SYSTEM level
SELECT pg_reload_conf();
```

Compile and run the extension in the `pgrx`-managed Postgres instance:

```sh
cargo pgrx run
```

> Note: To install extensions in your local Postgres, use `cargo pgrx install`. For more information, refer to the [docs](https://github.com/pgcentralfoundation/pgrx/tree/develop/cargo-pgrx#installing-your-extension-locally).

In the PostgreSQL shell:

```sql
DROP EXTENSION IF EXISTS pg_summarize;
CREATE EXTENSION pg_summarize;

-- Call the summarize function with a text input to get its summary
SELECT summarize('<This is the text to be summarized.>');

-- Create a new table 'blogs_summary' by summarizing the text from 'hexacluster_blogs'
CREATE TABLE blogs_summary AS SELECT blog_url, summarize(blogs_text) FROM hexacluster_blogs;

-- Create a new table called 'blogs_summary_4o' using the 'gpt-4o' model
SET pg_summarizer.model = 'gpt-4o';
CREATE TABLE blogs_summary_4o AS SELECT blog_url, summarize(blogs_text) FROM hexacluster_blogs;
```

## Thoughts

Exploring the `pgrx` crate further and reviewing its documentation can uncover more advanced features and samples. Rust's performance is near that of C/C++, and its extensive library ecosystem opens up numerous possibilities.

This project is just a demo, and there’s plenty of room for improvement. To explore further, you can consider implementing these enhancements and send your patches to [HexaCluster/pg_summarize](https://github.com/HexaCluster/pg_summarize):

- **Error Handling**: Improve robustness by handling API failures and edge cases more gracefully.
- **Customization**: Allow users to configure API request parameters directly through PostgreSQL settings.
- **Caching**: Implement caching to store frequent API responses, enhancing performance and reducing costs.
- **Security**: Ensure secure storage and access of API keys, integrating PostgreSQL's security features.
- **Scalability**: Optimize for efficient resource management and load balancing to handle growing usage.
- **Logging and Monitoring**: Add capabilities to track usage, performance, and errors for better maintenance.
- **Additional Integrations**: Extend functionality with other AI services like translation or sentiment analysis.
- **Community Engagement**: Open-source your extension to attract contributions and feedback from the community.

By integrating large language models (LLMs) like OpenAI's, you can innovate and build powerful tools directly within PostgreSQL, making it a more intelligent and capable database system.

## Conclusion

In this blog, we've walked through the process of creating a PostgreSQL extension using Rust and `pgrx`. We started with a simple "Hello, World!" function and then built a more complex function that integrates with the OpenAI API to summarize text. This example demonstrates the power and flexibility of extending PostgreSQL with Rust, opening up a world of possibilities for database functionality.

Feel free to expand on this foundation and explore other ways Rust can enhance your PostgreSQL experience. Happy coding!
