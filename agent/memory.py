# agent/memory.py
from config import VAULT_PATH, TOP_K
from vault.indexer import search
from vault.manager import extract_wikilinks


def _search_path(vault_path, link_name: str):
    """Find a wikilinked file anywhere in the vault."""
    for candidate in vault_path.rglob(f"{link_name}.md"):
        return candidate
    return None


_CORE_FILES = ["memory/user.md", "memory/code.md", "memory/skills.md"]


def get_context(messages: list[dict] | str) -> str:
    if isinstance(messages, str):
        query = messages
    else:
        user_turns = [m["content"][:300] for m in messages[-6:] if m["role"] == "user"]
        query = " ".join(user_turns)

    seen: set[str] = set()
    chunks: list[str] = []

    # Always inject core identity files first
    for cf in _CORE_FILES:
        full = VAULT_PATH / cf
        if not full.exists():
            continue
        seen.add(cf)
        chunks.append(f"### {cf}\n{full.read_text(encoding='utf-8')[:2000]}")

    # RAG results on top
    for r in search(query, top_k=TOP_K):
        path = r["path"]
        if path in seen:
            continue
        seen.add(path)
        chunks.append(f"### {path}\n{r['content']}")

        for link in extract_wikilinks(r["content"]):
            found = _search_path(VAULT_PATH, link)
            if found is None:
                continue
            rel = str(found.relative_to(VAULT_PATH))
            if rel in seen:
                continue
            seen.add(rel)
            chunks.append(f"### {rel}\n{found.read_text(encoding='utf-8')[:1000]}")

    return "\n\n---\n\n".join(chunks)
