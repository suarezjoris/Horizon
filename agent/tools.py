# agent/tools.py
import httpx
import html2text
from duckduckgo_search import DDGS

def web_search(query: str, n: int = 5) -> str:
    with DDGS() as ddgs:
        results = list(ddgs.text(query, max_results=n))
    lines = [f"- [{r['title']}]({r['href']})\n  {r['body']}" for r in results]
    return "\n".join(lines)

def fetch_url(url: str, max_chars: int = 8000) -> str:
    resp = httpx.get(url, follow_redirects=True, timeout=10)
    resp.raise_for_status()
    h = html2text.HTML2Text()
    h.ignore_links = False
    h.body_width = 0
    return h.handle(resp.text)[:max_chars]
