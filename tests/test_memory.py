# tests/test_memory.py
import sys
from unittest.mock import patch

SEARCH_RESULTS = [
    {"path": "memory/code.md", "content": "Python style simple [[user]]"},
    {"path": "memory/user.md",  "content": "Joris, dev Go/Python"},
]

def test_get_context_returns_content(tmp_vault):
    (tmp_vault / "memory" / "user.md").write_text("# User\nJoris", encoding="utf-8")
    sys.modules.pop("agent.memory", None)
    with patch("agent.memory.search", return_value=SEARCH_RESULTS), \
         patch("agent.memory.VAULT_PATH", tmp_vault):
        from agent.memory import get_context
        ctx = get_context("code python")
    assert "memory/code.md" in ctx
    assert "Python style simple" in ctx

def test_get_context_follows_wikilinks(tmp_vault):
    (tmp_vault / "memory").mkdir(exist_ok=True)
    (tmp_vault / "memory" / "user.md").write_text("# User\nJoris", encoding="utf-8")
    sys.modules.pop("agent.memory", None)
    with patch("agent.memory.search", return_value=SEARCH_RESULTS), \
         patch("agent.memory.VAULT_PATH", tmp_vault):
        from agent.memory import get_context
        ctx = get_context("code python")
    assert "Joris" in ctx  # contenu de user.md suivi via [[user]]

def test_get_context_ignores_missing_wikilinks(tmp_vault):
    results = [{"path": "memory/code.md", "content": "voir [[inexistant]]"}]
    sys.modules.pop("agent.memory", None)
    with patch("agent.memory.search", return_value=results), \
         patch("agent.memory.VAULT_PATH", tmp_vault):
        from agent.memory import get_context
        ctx = get_context("code")  # ne doit pas lever d'exception
    assert "memory/code.md" in ctx

def test_get_context_no_duplicate_paths(tmp_vault):
    results = [
        {"path": "memory/user.md", "content": "Joris [[user]]"},  # [[user]] → user.md déjà vu
    ]
    (tmp_vault / "memory" / "user.md").write_text("# User", encoding="utf-8")
    sys.modules.pop("agent.memory", None)
    with patch("agent.memory.search", return_value=results), \
         patch("agent.memory.VAULT_PATH", tmp_vault):
        from agent.memory import get_context
        ctx = get_context("user")
    assert ctx.count("memory/user.md") == 1  # pas de doublon
