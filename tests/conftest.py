import pytest
from pathlib import Path

@pytest.fixture
def tmp_vault(tmp_path):
    """Vault temporaire pour les tests."""
    vault = tmp_path / "vault"
    vault.mkdir()
    (vault / "memory").mkdir()
    (vault / "knowledge").mkdir()
    (vault / "conversations").mkdir()
    return vault
