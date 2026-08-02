//! Native tool registry and built-in web tool adapters.

use crate::ast::{Program, ToolDef, TypeExpr};
use regex::Regex;
use reqwest::blocking::Client;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use url::form_urlencoded;

type ToolHandler = Arc<dyn Fn(&Map<String, Value>) -> Result<Value, ToolError> + Send + Sync>;

#[derive(Debug, Error)]
pub enum ToolError {
    #[error("unknown tool '{0}'")]
    Unknown(String),
    #[error("tool '{tool}' is not declared in this program")]
    Undeclared { tool: String },
    #[error("tool '{tool}' argument '{argument}' is missing or has the wrong type")]
    InvalidArgument { tool: String, argument: String },
    #[error("tool '{tool}' returned a value incompatible with its declaration")]
    InvalidResult { tool: String },
    #[error("{tool} HTTP {status}: {detail}")]
    Http {
        tool: String,
        status: u16,
        detail: String,
    },
    #[error("{tool} network error: {detail}")]
    Network { tool: String, detail: String },
}

#[derive(Clone, Default)]
pub struct ToolRegistry {
    handlers: BTreeMap<String, ToolHandler>,
}

impl ToolRegistry {
    pub fn register<F>(&mut self, name: impl Into<String>, handler: F)
    where
        F: Fn(&Map<String, Value>) -> Result<Value, ToolError> + Send + Sync + 'static,
    {
        self.handlers.insert(name.into(), Arc::new(handler));
    }

    pub fn execute(
        &self,
        program: &Program,
        name: &str,
        args: &Map<String, Value>,
    ) -> Result<Value, ToolError> {
        let declaration = program
            .tools
            .get(name)
            .ok_or_else(|| ToolError::Undeclared { tool: name.into() })?;
        validate_args(program, declaration, args)?;
        let handler = self
            .handlers
            .get(name)
            .ok_or_else(|| ToolError::Unknown(name.into()))?;
        let result = handler(args)?;
        if !value_matches(program, &result, &declaration.return_type) {
            return Err(ToolError::InvalidResult { tool: name.into() });
        }
        Ok(result)
    }
}

pub fn default_tool_registry(timeout: Duration) -> Result<ToolRegistry, ToolError> {
    let client = Client::builder()
        .timeout(timeout)
        .user_agent(format!(
            "AGL/{} (+https://nanamanu.com/agl)",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .map_err(|e| ToolError::Network {
            tool: "web tools".into(),
            detail: e.to_string(),
        })?;
    let mut registry = ToolRegistry::default();
    let search_client = client.clone();
    registry.register("web_search", move |args| {
        let query = args.get("query").and_then(Value::as_str).ok_or_else(|| {
            ToolError::InvalidArgument {
                tool: "web_search".into(),
                argument: "query".into(),
            }
        })?;
        search(&search_client, query, 5)
    });
    registry.register("fetch_url", move |args| {
        let url =
            args.get("url")
                .and_then(Value::as_str)
                .ok_or_else(|| ToolError::InvalidArgument {
                    tool: "fetch_url".into(),
                    argument: "url".into(),
                })?;
        fetch(&client, url, 50_000).map(|text| json!({ "content": text }))
    });
    Ok(registry)
}

pub fn tool_definitions(program: &Program, names: &[String]) -> Vec<Value> {
    names
        .iter()
        .filter_map(|name| program.tools.get(name))
        .map(|tool| {
            let properties: Map<String, Value> = tool
                .params
                .iter()
                .map(|p| (p.name.clone(), type_schema(program, &p.ty)))
                .collect();
            json!({
                "type": "function",
                "name": tool.name,
                "description": format!("Execute the '{}' tool declared in AGL.", tool.name),
                "parameters": {
                    "type": "object",
                    "properties": properties,
                    "required": tool.params.iter().map(|p| &p.name).collect::<Vec<_>>(),
                    "additionalProperties": false
                }
            })
        })
        .collect()
}

pub fn type_schema(program: &Program, ty: &TypeExpr) -> Value {
    match ty {
        TypeExpr::String => json!({"type":"string"}),
        TypeExpr::Number => json!({"type":"number"}),
        TypeExpr::Bool => json!({"type":"boolean"}),
        TypeExpr::Failure => json!({
            "type":"object",
            "properties":{
                "kind":{"type":"string"},
                "message":{"type":"string"},
                "operation":{"anyOf":[{"type":"string"},{"type":"null"}]},
                "retryable":{"type":"boolean"}
            },
            "required":["kind","message","operation","retryable"],
            "additionalProperties":false
        }),
        TypeExpr::List(item) => json!({"type":"array","items":type_schema(program,item)}),
        TypeExpr::Option(item) => {
            json!({"anyOf":[type_schema(program,item),{"type":"null"}]})
        }
        TypeExpr::Result(ok, error) => json!({"oneOf":[
            {"type":"object","properties":{"$type":{"const":"Result"},"$variant":{"const":"Ok"},"value":type_schema(program,ok)},"required":["$type","$variant","value"],"additionalProperties":false},
            {"type":"object","properties":{"$type":{"const":"Result"},"$variant":{"const":"Err"},"error":type_schema(program,error)},"required":["$type","$variant","error"],"additionalProperties":false}
        ]}),
        TypeExpr::Obj(fields) => {
            let properties: Map<String, Value> = fields
                .iter()
                .map(|(name, ty)| (name.clone(), type_schema(program, ty)))
                .collect();
            json!({"type":"object","properties":properties,"required":fields.keys().collect::<Vec<_>>(),"additionalProperties":false})
        }
        TypeExpr::Record(name) => program
            .records
            .get(name)
            .map(|record| type_schema(program, &TypeExpr::Obj(record.fields.clone())))
            .unwrap_or_else(|| json!({})),
        TypeExpr::Union(name) => program
            .unions
            .get(name)
            .map(|union| {
                let variants: Vec<_> = union
                    .variants
                    .iter()
                    .map(|(variant, fields)| {
                        let mut properties = Map::from_iter([
                            ("$type".into(), json!({"const":name})),
                            ("$variant".into(), json!({"const":variant})),
                        ]);
                        properties.extend(fields.iter().map(|(field, ty)| {
                            (field.clone(), type_schema(program, ty))
                        }));
                        let mut required = vec!["$type", "$variant"];
                        required.extend(fields.keys().map(String::as_str));
                        json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
                    })
                    .collect();
                json!({"oneOf":variants})
            })
            .unwrap_or_else(|| json!({})),
        TypeExpr::Enum(name) => {
            json!({"type":"string","enum":program.enums.get(name).cloned().unwrap_or_default()})
        }
        TypeExpr::Alias(name) => program
            .aliases
            .get(name)
            .map(|x| type_schema(program, x))
            .unwrap_or_else(|| json!({})),
    }
}

fn search(client: &Client, query: &str, max_results: usize) -> Result<Value, ToolError> {
    let query_string = form_urlencoded::Serializer::new(String::new())
        .append_pair("q", query)
        .finish();
    let response = client
        .get(format!("https://html.duckduckgo.com/html/?{query_string}"))
        .send()
        .map_err(|e| ToolError::Network {
            tool: "web_search".into(),
            detail: e.to_string(),
        })?;
    let status = response.status();
    let body = response.text().map_err(|e| ToolError::Network {
        tool: "web_search".into(),
        detail: e.to_string(),
    })?;
    if !status.is_success() {
        return Err(ToolError::Http {
            tool: "web_search".into(),
            status: status.as_u16(),
            detail: body,
        });
    }
    Ok(Value::Array(parse_search_html(&body, max_results)))
}

fn fetch(client: &Client, url: &str, max_bytes: usize) -> Result<String, ToolError> {
    let response = client.get(url).send().map_err(|e| ToolError::Network {
        tool: "fetch_url".into(),
        detail: e.to_string(),
    })?;
    let status = response.status();
    let bytes = response.bytes().map_err(|e| ToolError::Network {
        tool: "fetch_url".into(),
        detail: e.to_string(),
    })?;
    let body = String::from_utf8_lossy(&bytes[..bytes.len().min(max_bytes)]).into_owned();
    if !status.is_success() {
        return Err(ToolError::Http {
            tool: "fetch_url".into(),
            status: status.as_u16(),
            detail: body,
        });
    }
    Ok(strip_tags(&body))
}

fn parse_search_html(html: &str, max_results: usize) -> Vec<Value> {
    let link = Regex::new(r#"(?s)<a[^>]*(?:class="result__a"[^>]*href="([^"]*)"|href="([^"]*)"[^>]*class="result__a")[^>]*>(.*?)</a>"#).unwrap();
    let snippet = Regex::new(r#"(?s)<a[^>]*class="result__snippet"[^>]*>(.*?)</a>"#).unwrap();
    let snippets: Vec<_> = snippet
        .captures_iter(html)
        .map(|c| strip_tags(&c[1]))
        .collect();
    link.captures_iter(html).take(max_results).enumerate().map(|(index,c)|{
        let raw=c.get(1).or_else(||c.get(2)).map(|x|x.as_str()).unwrap_or("");
        json!({"title":decode_entities(&strip_tags(&c[3])),"url":extract_redirect(raw),"snippet":decode_entities(snippets.get(index).map(String::as_str).unwrap_or(""))})
    }).collect()
}

fn extract_redirect(raw: &str) -> String {
    url::Url::parse(raw)
        .ok()
        .and_then(|u| {
            u.query_pairs()
                .find(|(k, _)| k == "uddg")
                .map(|(_, v)| v.into_owned())
        })
        .unwrap_or_else(|| raw.into())
}
fn strip_tags(text: &str) -> String {
    let scripts = Regex::new(r"(?is)<(?:script|style).*?>.*?</(?:script|style)>")
        .unwrap()
        .replace_all(text, " ");
    let tags = Regex::new(r"(?s)<[^>]+>")
        .unwrap()
        .replace_all(&scripts, " ");
    Regex::new(r"\s+")
        .unwrap()
        .replace_all(&tags, " ")
        .trim()
        .into()
}
fn decode_entities(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

fn validate_args(
    program: &Program,
    tool: &ToolDef,
    args: &Map<String, Value>,
) -> Result<(), ToolError> {
    if args.len() != tool.params.len() {
        return Err(ToolError::InvalidArgument {
            tool: tool.name.clone(),
            argument: "argument set".into(),
        });
    }
    for p in &tool.params {
        if !args
            .get(&p.name)
            .is_some_and(|v| value_matches(program, v, &p.ty))
        {
            return Err(ToolError::InvalidArgument {
                tool: tool.name.clone(),
                argument: p.name.clone(),
            });
        }
    }
    Ok(())
}
fn value_matches(program: &Program, value: &Value, ty: &TypeExpr) -> bool {
    match ty {
        TypeExpr::String => value.is_string(),
        TypeExpr::Number => value.is_number(),
        TypeExpr::Bool => value.is_boolean(),
        TypeExpr::Failure => value.as_object().is_some_and(|object| {
            object.get("kind").is_some_and(Value::is_string)
                && object.get("message").is_some_and(Value::is_string)
                && object
                    .get("operation")
                    .is_some_and(|value| value.is_null() || value.is_string())
                && object.get("retryable").is_some_and(Value::is_boolean)
        }),
        TypeExpr::List(x) => value
            .as_array()
            .is_some_and(|v| v.iter().all(|v| value_matches(program, v, x))),
        TypeExpr::Option(x) => value.is_null() || value_matches(program, value, x),
        TypeExpr::Result(ok, error) => value.as_object().is_some_and(|object| {
            object.get("$type").and_then(Value::as_str) == Some("Result")
                && match object.get("$variant").and_then(Value::as_str) {
                    Some("Ok") => object
                        .get("value")
                        .is_some_and(|value| value_matches(program, value, ok)),
                    Some("Err") => object
                        .get("error")
                        .is_some_and(|value| value_matches(program, value, error)),
                    _ => false,
                }
        }),
        TypeExpr::Obj(fs) => value.as_object().is_some_and(|o| {
            fs.iter()
                .all(|(k, t)| o.get(k).is_some_and(|v| value_matches(program, v, t)))
        }),
        TypeExpr::Record(name) => program.records.get(name).is_some_and(|record| {
            value.as_object().is_some_and(|object| {
                record.fields.iter().all(|(field, ty)| {
                    object
                        .get(field)
                        .is_some_and(|value| value_matches(program, value, ty))
                })
            })
        }),
        TypeExpr::Union(name) => program.unions.get(name).is_some_and(|union| {
            value.as_object().is_some_and(|object| {
                object.get("$type").and_then(Value::as_str) == Some(name)
                    && object
                        .get("$variant")
                        .and_then(Value::as_str)
                        .and_then(|variant| union.variants.get(variant))
                        .is_some_and(|fields| {
                            fields.iter().all(|(field, ty)| {
                                object
                                    .get(field)
                                    .is_some_and(|value| value_matches(program, value, ty))
                            })
                        })
            })
        }),
        TypeExpr::Enum(n) => value.as_str().is_some_and(|v| {
            program
                .enums
                .get(n)
                .is_some_and(|xs| xs.iter().any(|x| x == v))
        }),
        TypeExpr::Alias(n) => program
            .aliases
            .get(n)
            .is_some_and(|t| value_matches(program, value, t)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_search_results_and_redirects() {
        let html = r#"<a class="result__a" href="https://duckduckgo.com/l/?uddg=https%3A%2F%2Frust-lang.org">Rust &amp; Safety</a><a class="result__snippet">Fast <b>systems</b> language</a>"#;
        let hits = parse_search_html(html, 5);
        assert_eq!(hits[0]["title"], "Rust & Safety");
        assert_eq!(hits[0]["url"], "https://rust-lang.org");
        assert_eq!(hits[0]["snippet"], "Fast systems language")
    }
    #[test]
    fn generates_enum_schema() {
        let p = Program {
            enums: BTreeMap::from([("Tone".into(), vec!["formal".into(), "casual".into()])]),
            ..Program::default()
        };
        assert_eq!(
            type_schema(&p, &TypeExpr::Enum("Tone".into())),
            json!({"type":"string","enum":["formal","casual"]})
        )
    }
}
