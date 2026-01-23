"""Pytest fixtures for the Task Manager API tests."""

import pytest
from fastapi.testclient import TestClient

from app.main import app
from app.store import store


@pytest.fixture
def client() -> TestClient:
    """Create a test client for the API."""
    store.clear()
    return TestClient(app)
