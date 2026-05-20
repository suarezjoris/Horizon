# tests/test_core.py
from unittest.mock import patch, MagicMock, call

def _make_ollama_response(content: str):
    """Mock d'une réponse Ollama non-streamée."""
    return {"message": {"content": content}}

def _make_stream_chunks(content: str):
    """Mock d'un stream Ollama token par token."""
    return [{"message": {"content": c}} for c in content]

def test_run_agent_simple_response(mocker):
    mocker.patch("agent.core.get_context", return_value="contexte mémoire")
    mock_client = MagicMock()
    mock_client.chat.return_value = _make_stream_chunks("Bonjour !")
    mocker.patch("agent.core.ollama.Client", return_value=mock_client)
    mocker.patch("agent.core._save_memory")

    from agent.core import run_agent
    result = run_agent([{"role": "user", "content": "Salut"}])
    assert result == "Bonjour !"

def test_run_agent_calls_web_search(mocker):
    mocker.patch("agent.core.get_context", return_value="")
    mock_client = MagicMock()
    mock_client.chat.side_effect = [
        _make_stream_chunks('ACTION: web_search("CVE-2024-1234")'),
        _make_stream_chunks("Voici les résultats du CVE."),
    ]
    mocker.patch("agent.core.ollama.Client", return_value=mock_client)
    mocker.patch("agent.core.web_search", return_value="CVE résultats")
    mocker.patch("agent.core._save_memory")

    from agent.core import run_agent
    result = run_agent([{"role": "user", "content": "Infos sur CVE-2024-1234"}])
    assert result == "Voici les résultats du CVE."

def test_run_agent_calls_fetch_url(mocker):
    mocker.patch("agent.core.get_context", return_value="")
    mock_client = MagicMock()
    mock_client.chat.side_effect = [
        _make_stream_chunks('ACTION: fetch_url("https://example.com")'),
        _make_stream_chunks("Contenu de la page."),
    ]
    mocker.patch("agent.core.ollama.Client", return_value=mock_client)
    mocker.patch("agent.core.fetch_url", return_value="contenu html parsé")
    mocker.patch("agent.core._save_memory")

    from agent.core import run_agent
    result = run_agent([{"role": "user", "content": "Lis cette page"}])
    assert result == "Contenu de la page."

def test_run_agent_max_iterations(mocker):
    """Vérifie que le loop s'arrête après 5 itérations même sans réponse finale."""
    mocker.patch("agent.core.get_context", return_value="")
    mock_client = MagicMock()
    mock_client.chat.return_value = _make_stream_chunks('ACTION: web_search("loop infini")')
    mocker.patch("agent.core.ollama.Client", return_value=mock_client)
    mocker.patch("agent.core.web_search", return_value="résultat")
    mocker.patch("agent.core._save_memory")

    from agent.core import run_agent
    run_agent([{"role": "user", "content": "test"}])
    assert mock_client.chat.call_count == 5

def test_save_memory_writes_append(mocker, tmp_vault):
    mock_client = MagicMock()
    mock_client.chat.return_value = _make_ollama_response(
        '[{"path": "memory/user.md", "append": "- nouveau fait"}]'
    )
    mocker.patch("agent.core.write_note")
    mock_append = mocker.patch("agent.core.append_note")
    mocker.patch("agent.core.read_note", return_value="# User")

    from agent.core import _save_memory
    _save_memory([{"role": "user", "content": "test"}], "réponse", mock_client)
    mock_append.assert_called_once_with("memory/user.md", "- nouveau fait")

def test_save_memory_writes_content(mocker):
    mock_client = MagicMock()
    mock_client.chat.return_value = _make_ollama_response(
        '[{"path": "knowledge/python.md", "content": "# Python\n- tips"}]'
    )
    mock_write = mocker.patch("agent.core.write_note")
    mocker.patch("agent.core.append_note")
    mocker.patch("agent.core.read_note", return_value="")

    from agent.core import _save_memory
    _save_memory([{"role": "user", "content": "test"}], "réponse", mock_client)
    mock_write.assert_called_once_with("knowledge/python.md", "# Python\n- tips")
