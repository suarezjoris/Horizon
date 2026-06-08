import sys
import json
from ddgs import DDGS

def search(query):
    try:
        results = DDGS().text(query, max_results=5)
        if not results:
            return "No results found on the web."
        
        output = ["Results from the web:\n"]
        for i, res in enumerate(results):
            output.append(f"{i+1}. {res['title']}\n   {res['body']}")
        return "\n\n".join(output) + "\n"
    except Exception as e:
        return f"Search error: {str(e)}"

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: python search_web.py <query>")
        sys.exit(1)
    
    query = sys.argv[1]
    print(search(query))
