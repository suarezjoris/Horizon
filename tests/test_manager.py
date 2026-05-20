# tests/test_manager.py
import pytest
from pathlib import Path
from unittest.mock import patch

def test_write_and_read_note(tmp_vault):
    with patch("vault.manager.VAULT_PATH", tmp_vault):
        from vault.manager import write_note, read_note
        write_note("memory/user.md", "# User\n- Joris")
        assert read_note("memory/user.md") == "# User\n- Joris"

def test_write_creates_parent_dirs(tmp_vault):
    with patch("vault.manager.VAULT_PATH", tmp_vault):
        from vault.manager import write_note, read_note
        write_note("knowledge/python/tips.md", "content")
        assert read_note("knowledge/python/tips.md") == "content"

def test_read_missing_note_raises(tmp_vault):
    with patch("vault.manager.VAULT_PATH", tmp_vault):
        from vault.manager import read_note
        with pytest.raises(FileNotFoundError):
            read_note("missing.md")

def test_list_notes(tmp_vault):
    with patch("vault.manager.VAULT_PATH", tmp_vault):
        from vault.manager import write_note, list_notes
        write_note("memory/user.md", "a")
        write_note("knowledge/python.md", "b")
        paths = list_notes()
        assert len(paths) == 2

def test_extract_wikilinks():
    from vault.manager import extract_wikilinks
    content = "Voir [[code]] et [[python]] pour plus d'infos. Aussi [[user]]."
    links = extract_wikilinks(content)
    assert links == ["code", "python", "user"]

def test_extract_wikilinks_empty():
    from vault.manager import extract_wikilinks
    assert extract_wikilinks("Pas de liens ici.") == []

def test_append_to_note(tmp_vault):
    with patch("vault.manager.VAULT_PATH", tmp_vault):
        from vault.manager import write_note, append_note, read_note
        write_note("memory/user.md", "# User")
        append_note("memory/user.md", "- nouveau fait")
        assert read_note("memory/user.md") == "# User\n- nouveau fait"

def test_append_creates_if_missing(tmp_vault):
    with patch("vault.manager.VAULT_PATH", tmp_vault):
        from vault.manager import append_note, read_note
        append_note("memory/new.md", "- premier fait")
        assert read_note("memory/new.md") == "- premier fait"
