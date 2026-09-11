use crate::{
    analyze_program, check_program, format_pipeline, format_program, infer_program_effects,
    parse_program,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::{BufRead, Read, Write};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompilerRequest {
    pub id: Value,
    pub method: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub pipeline: Option<String>,
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(default)]
    pub replacement: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompilerResponse {
    pub id: Value,
    pub ok: bool,
    pub result: Value,
}

pub fn handle(request: CompilerRequest) -> CompilerResponse {
    let result = (|| -> Result<Value, String> {
        let program = parse_program(&request.source).map_err(|error| error.to_string())?;
        match request.method.as_str() {
            "check" => {
                check_program(&program).map_err(|error| error.to_string())?;
                Ok(json!({"diagnostics":analyze_program(&program)}))
            }
            "format" => Ok(json!({"source":format_program(&program)})),
            "effects" => Ok(json!(infer_program_effects(&program))),
            "lower" => {
                let name = request.pipeline.ok_or("pipeline is required")?;
                Ok(
                    json!({"source":format_pipeline(program.pipelines.get(&name).ok_or("unknown pipeline")?)}),
                )
            }
            "completion" => {
                let mut items = vec![
                    "agent", "task", "tool", "pipeline", "record", "union", "match", "parallel",
                    "race", "eval", "import", "public",
                ];
                items.extend(program.tasks.keys().map(String::as_str));
                items.extend(program.pipelines.keys().map(String::as_str));
                items.sort_unstable();
                items.dedup();
                Ok(json!(items))
            }
            "hover" => {
                let symbol = request.symbol.ok_or("symbol is required")?;
                let api = crate::documentation::api_interface(&program);
                Ok(api
                    .exports
                    .get(&symbol)
                    .map(|item| json!(item))
                    .unwrap_or(Value::Null))
            }
            "definition" => {
                let symbol = request.symbol.ok_or("symbol is required")?;
                Ok(find_definition(&request.source, &symbol)
                    .map(|(line, col)| json!({"line":line,"character":col}))
                    .unwrap_or(Value::Null))
            }
            "references" => {
                let symbol = request.symbol.ok_or("symbol is required")?;
                Ok(json!(find_occurrences(&request.source, &symbol)))
            }
            "rename" => {
                let symbol = request.symbol.ok_or("symbol is required")?;
                let replacement = request.replacement.ok_or("replacement is required")?;
                Ok(
                    json!({"edits":find_occurrences(&request.source, &symbol).into_iter().map(|(line,character)|json!({"line":line,"character":character,"length":symbol.len(),"replacement":replacement})).collect::<Vec<_>>()}),
                )
            }
            method => Err(format!("unknown compiler method '{method}'")),
        }
    })();
    match result {
        Ok(result) => CompilerResponse {
            id: request.id,
            ok: true,
            result,
        },
        Err(message) => CompilerResponse {
            id: request.id,
            ok: false,
            result: json!({"message":message}),
        },
    }
}

pub fn serve_json_lines() -> Result<(), Box<dyn std::error::Error>> {
    let input = std::io::stdin();
    let mut output = std::io::stdout();
    for line in input.lock().lines() {
        let request: CompilerRequest = serde_json::from_str(&line?)?;
        serde_json::to_writer(&mut output, &handle(request))?;
        writeln!(output)?;
        output.flush()?;
    }
    Ok(())
}

pub fn serve_lsp() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout();
    let mut documents = std::collections::BTreeMap::<String, String>::new();
    loop {
        let mut content_length = None;
        loop {
            let mut line = String::new();
            if input.read_line(&mut line)? == 0 {
                return Ok(());
            }
            if line == "\r\n" || line == "\n" {
                break;
            }
            if let Some(value) = line.strip_prefix("Content-Length:") {
                content_length = Some(value.trim().parse::<usize>()?);
            }
        }
        let mut body = vec![0; content_length.ok_or("missing Content-Length")?];
        input.read_exact(&mut body)?;
        let request: Value = serde_json::from_slice(&body)?;
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request.get("method").and_then(Value::as_str).unwrap_or("");
        if matches!(method, "textDocument/didOpen" | "textDocument/didChange") {
            let uri = request
                .pointer("/params/textDocument/uri")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let text = if method.ends_with("didOpen") {
                request.pointer("/params/textDocument/text")
            } else {
                request.pointer("/params/contentChanges/0/text")
            }
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
            documents.insert(uri.clone(), text.clone());
            let diagnostics = source_diagnostics(&text);
            write_lsp(
                &mut output,
                &json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"uri":uri,"diagnostics":diagnostics}}),
            )?;
            continue;
        }
        let uri = request
            .pointer("/params/textDocument/uri")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let source = documents.get(uri).cloned().unwrap_or_default();
        let symbol = request.pointer("/params/position").and_then(|position| {
            word_at(
                &source,
                position["line"].as_u64().unwrap_or(0) as usize,
                position["character"].as_u64().unwrap_or(0) as usize,
            )
        });
        let compiler = |name: &str| {
            handle(CompilerRequest {
                id: id.clone(),
                method: name.into(),
                source: source.clone(),
                pipeline: None,
                symbol: symbol.clone(),
                replacement: request
                    .pointer("/params/newName")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            })
            .result
        };
        let result = match method {
            "initialize" => {
                json!({"capabilities":{"textDocumentSync":1,"hoverProvider":true,"definitionProvider":true,"referencesProvider":true,"renameProvider":true,"completionProvider":{},"documentFormattingProvider":true}})
            }
            "shutdown" => Value::Null,
            "textDocument/completion" => json!(
                compiler("completion")
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|label| json!({"label":label,"kind":6}))
                    .collect::<Vec<_>>()
            ),
            "textDocument/hover" => {
                json!({"contents":{"kind":"markdown","value":format!("```json\n{}\n```", compiler("hover"))}})
            }
            "textDocument/definition" => lsp_locations(uri, &json!([compiler("definition")])),
            "textDocument/references" => lsp_locations(uri, &compiler("references")),
            "textDocument/rename" => json!({"changes":{uri:lsp_edits(&compiler("rename"))}}),
            "textDocument/formatting" => {
                json!([{"range":{"start":{"line":0,"character":0},"end":{"line":u32::MAX,"character":0}},"newText":compiler("format")["source"]}])
            }
            _ => json!({}),
        };
        if !id.is_null() {
            write_lsp(
                &mut output,
                &json!({"jsonrpc":"2.0","id":id,"result":result}),
            )?;
        }
        if method == "exit" {
            return Ok(());
        }
    }
}

fn write_lsp(output: &mut impl Write, value: &Value) -> Result<(), Box<dyn std::error::Error>> {
    let response = serde_json::to_vec(value)?;
    write!(output, "Content-Length: {}\r\n\r\n", response.len())?;
    output.write_all(&response)?;
    output.flush()?;
    Ok(())
}
fn source_diagnostics(source: &str) -> Vec<Value> {
    match parse_program(source) { Err(error) => vec![json!({"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":1}},"severity":1,"code":error.code(),"message":error.to_string()})], Ok(program) => match check_program(&program) { Err(error) => vec![json!({"range":{"start":{"line":error.line.saturating_sub(1),"character":error.col.saturating_sub(1)},"end":{"line":error.line.saturating_sub(1),"character":error.col}},"severity":1,"code":error.code(),"message":error.to_string()})], Ok(()) => analyze_program(&program).into_iter().map(|warning|json!({"range":{"start":{"line":warning.line.saturating_sub(1),"character":warning.col.saturating_sub(1)},"end":{"line":warning.line.saturating_sub(1),"character":warning.col}},"severity":2,"code":warning.code,"message":warning.message})).collect() } }
}
fn word_at(source: &str, line: usize, character: usize) -> Option<String> {
    let text = source.lines().nth(line)?;
    let bytes = text.as_bytes();
    let mut start = character.min(bytes.len());
    let mut end = start;
    while start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
        start -= 1;
    }
    while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
        end += 1;
    }
    (start < end).then(|| text[start..end].into())
}
fn lsp_locations(uri: &str, values: &Value) -> Value {
    json!(values.as_array().into_iter().flatten().filter_map(|value| { let line = value.get("line").and_then(Value::as_u64).or_else(||value.get(0).and_then(Value::as_u64))?; let character = value.get("character").and_then(Value::as_u64).or_else(||value.get(1).and_then(Value::as_u64))?; Some(json!({"uri":uri,"range":{"start":{"line":line,"character":character},"end":{"line":line,"character":character+1}}})) }).collect::<Vec<_>>())
}
fn lsp_edits(value: &Value) -> Value {
    json!(value.get("edits").and_then(Value::as_array).into_iter().flatten().filter_map(|edit| { let line = edit.get("line")?.as_u64()?; let character = edit.get("character")?.as_u64()?; let length = edit.get("length")?.as_u64()?; Some(json!({"range":{"start":{"line":line,"character":character},"end":{"line":line,"character":character+length}},"newText":edit.get("replacement").cloned().unwrap_or(Value::String(String::new()))})) }).collect::<Vec<_>>())
}

pub fn shell_completion(shell: &str) -> Result<String, String> {
    let commands = "lsp protocol package api-compare completions repl --input --test --check --output-trace --adapter --trace-live --lower --effects --deployment --eval --update-baseline --event-store --execution-id --resume --approval --format --docs --api --policy --summary";
    match shell {
        "bash" => Ok(format!("complete -W '{commands}' agl\n")),
        "zsh" => Ok(format!("compdef '_arguments *: :({commands})' agl\n")),
        "fish" => Ok(commands
            .split_whitespace()
            .map(|flag| format!("complete -c agl -l {}", flag.trim_start_matches('-')))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"),
        _ => Err("shell must be bash, zsh, or fish".into()),
    }
}

fn find_occurrences(source: &str, symbol: &str) -> Vec<(usize, usize)> {
    source
        .lines()
        .enumerate()
        .flat_map(|(line, text)| {
            text.match_indices(symbol)
                .filter(move |(column, _)| {
                    let before = text[..*column].chars().next_back();
                    let after = text[*column + symbol.len()..].chars().next();
                    !before.is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
                        && !after.is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
                })
                .map(move |(column, _)| (line, column))
        })
        .collect()
}
fn find_definition(source: &str, symbol: &str) -> Option<(usize, usize)> {
    let declarations = [
        "agent", "tool", "task", "pipeline", "record", "union", "enum", "type", "eval",
    ];
    source.lines().enumerate().find_map(|(line, text)| {
        declarations.iter().find_map(|kind| {
            let needle = format!("{kind} {symbol}");
            text.find(&needle)
                .map(|column| (line, column + kind.len() + 1))
        })
    })
}
