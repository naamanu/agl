# Testing

AgentLang has built-in support for in-language testing via `assert` statements and `test` blocks.

## Assert statement

```agentlang
assert <expr>, "<message>";
```

Evaluates the expression (must be `Bool`). If `false`, execution halts with an assertion error containing the message.

```agentlang
assert final.title != "", "Title must not be empty";
assert result.score == 100, "Score should be 100";
```

Asserts can appear in pipelines (as quality gates) or in test blocks (as test expectations).

## Test blocks

```agentlang
test "<name>" {
  <statements>
}
```

Test blocks are top-level declarations alongside agents, tasks, and pipelines. They contain pipeline-like statements (`let`, `run`, `assert`, etc.) and run in an isolated scope.

Test blocks are **only executed** when the `--test` flag is passed. During normal pipeline execution, they are parsed and type-checked but not run.

## Example

From `examples/showcase_all_features.agent`:

```agentlang
test "merge_drafts combines two drafts" {
  let result = run merge_drafts with {
    draft_a: "Part A.",
    draft_b: "Part B.",
    word_count_a: 100,
    word_count_b: 200
  };
  assert result.total_words == 300, "Total words should be sum of both";
  assert result.article != "", "Merged article should not be empty";
}

test "fallback_enrich produces content" {
  let result = run fallback_enrich with { query: "test query" };
  assert result.extra != "", "Fallback should produce non-empty extra content";
}

test "countdown reaches done" {
  let state = run countdown with { current: 3 };
  assert state.next == 2, "3 -> next should be 2";
  assert state.done == false, "Should not be done at next=2";
  let state = run countdown with { current: state.next };
  assert state.next == 1, "2 -> next should be 1";
  let state = run countdown with { current: state.next };
  assert state.next == 0, "1 -> next should be 0";
  assert state.done == true, "Should be done when next=0";
}
```

## Running tests

```bash
python main.py examples/showcase_all_features.agent --test
```

The `--test` flag runs all test blocks in the file. No pipeline name is needed.

### Output format

Each test prints its name and pass/fail status:

```
PASS  merge_drafts combines two drafts
PASS  fallback_enrich produces content
PASS  countdown reaches done

3 passed, 0 failed
```

### Exit codes

| Code | Meaning |
|---|---|
| `0` | All tests passed |
| `1` | One or more tests failed |

## Using with plugins

Test blocks use the same task registry as normal execution. If your tests depend on plugin-provided handlers, load the plugin:

```bash
python main.py examples/showcase_all_features.agent --test \
  --plugin examples/showcase_plugin.py
```

## Next: [Plugins](plugins.md)
