"""Sample plugin for showcase_all_features.agent.

Provides handlers for non-agent tasks: merge_drafts, risky_enrich, fallback_enrich.

Usage:
  python main.py examples/showcase_all_features.agent produce \
    '{"topic":"AI safety"}' --plugin examples/showcase_plugin.py
"""


def register(registry):
    def merge_drafts(args, _agent):
        a = args["draft_a"]
        b = args["draft_b"]
        wa = args["word_count_a"]
        wb = args["word_count_b"]
        merged = f"{a}\n\n{b}".strip() if b else a
        sections = [s for s in [a, b] if s]
        return {
            "article": merged,
            "sections": sections,
            "total_words": wa + wb,
        }

    def risky_enrich(args, _agent):
        # Simulate intermittent failure
        topic = args["topic"]
        if "fail" in topic.lower():
            raise RuntimeError(f"Enrichment service unavailable for: {topic}")
        return {"extra": f"[Enriched context for '{topic}']"}

    def fallback_enrich(args, _agent):
        return {"extra": f"[Fallback content: {args['query'][:80]}]"}

    registry.register_task("merge_drafts", merge_drafts)
    registry.register_task("risky_enrich", risky_enrich)
    registry.register_task("fallback_enrich", fallback_enrich)
