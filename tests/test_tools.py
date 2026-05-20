# tests/test_tools.py
from unittest.mock import patch, MagicMock

def test_web_search_returns_formatted_results():
    mock_results = [
        {"title": "CVE-2024-1234", "href": "https://nvd.nist.gov/1234", "body": "Buffer overflow"},
        {"title": "Exploit DB", "href": "https://exploit-db.com/5678", "body": "PoC disponible"},
    ]
    with patch("agent.tools.DDGS") as mock_ddgs:
        mock_ddgs.return_value.__enter__.return_value.text.return_value = mock_results
        from agent.tools import web_search
        result = web_search("CVE-2024-1234")
    assert "CVE-2024-1234" in result
    assert "https://nvd.nist.gov/1234" in result
    assert "Buffer overflow" in result

def test_web_search_empty_results():
    with patch("agent.tools.DDGS") as mock_ddgs:
        mock_ddgs.return_value.__enter__.return_value.text.return_value = []
        from agent.tools import web_search
        result = web_search("requête vide")
    assert result == ""

def test_fetch_url_extracts_text():
    mock_response = MagicMock()
    mock_response.text = "<html><body><h1>Titre</h1><p>Contenu important</p></body></html>"
    with patch("agent.tools.httpx.get", return_value=mock_response):
        from agent.tools import fetch_url
        result = fetch_url("https://example.com")
    assert "Titre" in result
    assert "Contenu important" in result

def test_fetch_url_truncates_at_max_chars():
    mock_response = MagicMock()
    mock_response.text = "<html><body>" + "x" * 20000 + "</body></html>"
    with patch("agent.tools.httpx.get", return_value=mock_response):
        from agent.tools import fetch_url
        result = fetch_url("https://example.com", max_chars=8000)
    assert len(result) <= 8000
