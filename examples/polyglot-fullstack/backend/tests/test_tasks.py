"""Tests for the task CRUD endpoints."""

from uuid import uuid4

from fastapi.testclient import TestClient


def test_list_tasks_empty(client: TestClient) -> None:
    """Test listing tasks when none exist."""
    response = client.get("/api/tasks")
    assert response.status_code == 200
    assert response.json() == []


def test_create_task(client: TestClient) -> None:
    """Test creating a new task."""
    response = client.post("/api/tasks", json={"title": "Test task"})
    assert response.status_code == 201
    data = response.json()
    assert data["title"] == "Test task"
    assert data["completed"] is False
    assert "id" in data
    assert "created_at" in data
    assert "updated_at" in data


def test_create_task_empty_title(client: TestClient) -> None:
    """Test that empty title is rejected."""
    response = client.post("/api/tasks", json={"title": ""})
    assert response.status_code == 422


def test_create_task_title_too_long(client: TestClient) -> None:
    """Test that too-long title is rejected."""
    response = client.post("/api/tasks", json={"title": "x" * 201})
    assert response.status_code == 422


def test_get_task(client: TestClient) -> None:
    """Test retrieving a specific task."""
    # Create a task first
    create_response = client.post("/api/tasks", json={"title": "Find me"})
    task_id = create_response.json()["id"]

    # Retrieve it
    response = client.get(f"/api/tasks/{task_id}")
    assert response.status_code == 200
    assert response.json()["title"] == "Find me"


def test_get_task_not_found(client: TestClient) -> None:
    """Test retrieving a non-existent task."""
    fake_id = uuid4()
    response = client.get(f"/api/tasks/{fake_id}")
    assert response.status_code == 404
    assert response.json()["detail"] == "Task not found"


def test_update_task_title(client: TestClient) -> None:
    """Test updating a task's title."""
    # Create a task
    create_response = client.post("/api/tasks", json={"title": "Original"})
    task_id = create_response.json()["id"]

    # Update it
    response = client.patch(f"/api/tasks/{task_id}", json={"title": "Updated"})
    assert response.status_code == 200
    assert response.json()["title"] == "Updated"


def test_update_task_completed(client: TestClient) -> None:
    """Test marking a task as completed."""
    # Create a task
    create_response = client.post("/api/tasks", json={"title": "Complete me"})
    task_id = create_response.json()["id"]
    assert create_response.json()["completed"] is False

    # Mark as completed
    response = client.patch(f"/api/tasks/{task_id}", json={"completed": True})
    assert response.status_code == 200
    assert response.json()["completed"] is True


def test_update_task_not_found(client: TestClient) -> None:
    """Test updating a non-existent task."""
    fake_id = uuid4()
    response = client.patch(f"/api/tasks/{fake_id}", json={"title": "Nope"})
    assert response.status_code == 404


def test_delete_task(client: TestClient) -> None:
    """Test deleting a task."""
    # Create a task
    create_response = client.post("/api/tasks", json={"title": "Delete me"})
    task_id = create_response.json()["id"]

    # Delete it
    response = client.delete(f"/api/tasks/{task_id}")
    assert response.status_code == 204

    # Verify it's gone
    get_response = client.get(f"/api/tasks/{task_id}")
    assert get_response.status_code == 404


def test_delete_task_not_found(client: TestClient) -> None:
    """Test deleting a non-existent task."""
    fake_id = uuid4()
    response = client.delete(f"/api/tasks/{fake_id}")
    assert response.status_code == 404


def test_list_tasks_after_creating(client: TestClient) -> None:
    """Test that created tasks appear in the list."""
    # Create some tasks
    client.post("/api/tasks", json={"title": "Task 1"})
    client.post("/api/tasks", json={"title": "Task 2"})
    client.post("/api/tasks", json={"title": "Task 3"})

    # List them
    response = client.get("/api/tasks")
    assert response.status_code == 200
    tasks = response.json()
    assert len(tasks) == 3

    # Verify newest first (default sort)
    titles = [t["title"] for t in tasks]
    assert titles == ["Task 3", "Task 2", "Task 1"]
