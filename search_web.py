import sys
import re
import requests
from ddgs import DDGS
from readability import Document

HEADERS = {"User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36"}
MAX_PAGE_CHARS = 3000
FETCH_TIMEOUT = 6

def fetch_page(url):
    try:
        r = requests.get(url, timeout=FETCH_TIMEOUT, headers=HEADERS)
        r.raise_for_status()
        doc = Document(r.text)
        text = doc.summary(html_partial=True)
        text = re.sub(r'<[^>]+>', ' ', text)
        text = re.sub(r'\s+', ' ', text).strip()
        return text[:MAX_PAGE_CHARS] if len(text) > 100 else None
    except Exception:
        return None

def search(query):
    try:
        results = DDGS().text(query, max_results=6)
        if not results:
            return "No results found on the web."

        output = ["Results from the web:\n"]
        for i, res in enumerate(results[:4]):
            title = res.get('title', 'No title')
            url = res.get('url', '')
            snippet = res.get('body', '')

            content = fetch_page(url) if url else None
            body = content if content else snippet

            source = f"[{title}]({url})" if url else title
            output.append(f"{i+1}. {source}\n{body}")

        return "\n\n---\n\n".join(output) + "\n"
    except Exception as e:
        return f"Search error: {str(e)}"

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: python search_web.py <query>")
        sys.exit(1)

    print(search(sys.argv[1]))
