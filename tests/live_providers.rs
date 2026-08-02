use agl::adapters::{AnthropicClient, CompletionRequest, ModelClient, OpenAiClient};

const MARKER: &str = "AGL_SMOKE_OK";

#[test]
#[ignore = "requires OPENAI_API_KEY and makes a billable network request"]
fn openai_live_completion() {
    let Ok(key) = std::env::var("OPENAI_API_KEY") else {
        eprintln!("OPENAI_API_KEY is not set; skipping OpenAI live smoke test");
        return;
    };
    let model = std::env::var("AGL_OPENAI_MODEL").unwrap_or_else(|_| "gpt-5.6-sol".into());
    let client = OpenAiClient::new(key).unwrap();
    let output = client
        .complete(CompletionRequest {
            model: &model,
            prompt: "Reply with exactly AGL_SMOKE_OK and no other text.",
            system: None,
            max_output_tokens: Some(32),
            reasoning_effort: Some("low"),
            idempotency_key: None,
            cancellation: None,
        })
        .unwrap();
    assert!(
        output.contains(MARKER),
        "unexpected OpenAI output: {output}"
    );
}

#[test]
#[ignore = "requires ANTHROPIC_API_KEY and makes a billable network request"]
fn anthropic_live_completion() {
    let Ok(key) = std::env::var("ANTHROPIC_API_KEY") else {
        eprintln!("ANTHROPIC_API_KEY is not set; skipping Anthropic live smoke test");
        return;
    };
    let model =
        std::env::var("AGL_ANTHROPIC_MODEL").unwrap_or_else(|_| "claude-haiku-4-5-20251001".into());
    let client = AnthropicClient::new(key).unwrap();
    let output = client
        .complete(CompletionRequest {
            model: &model,
            prompt: "Reply with exactly AGL_SMOKE_OK and no other text.",
            system: None,
            max_output_tokens: Some(32),
            reasoning_effort: None,
            idempotency_key: None,
            cancellation: None,
        })
        .unwrap();
    assert!(
        output.contains(MARKER),
        "unexpected Anthropic output: {output}"
    );
}
