"""In-memory task storage.

In a real application, this would be replaced with a database.
This simple implementation demonstrates the API without external dependencies.
"""

from datetime import UTC, datetime
from uuid import UUID, uuid4

from app.models import Task, TaskCreate, TaskUpdate


class TaskStore:
    """Simple in-memory task storage."""

    def __init__(self) -> None:
        """Initialize an empty task store."""
        self._tasks: dict[UUID, Task] = {}

    def list_all(self) -> list[Task]:
        """Return all tasks sorted by creation time (newest first)."""
        return sorted(self._tasks.values(), key=lambda t: t.created_at, reverse=True)

    def get(self, task_id: UUID) -> Task | None:
        """Get a task by its ID, or None if not found."""
        return self._tasks.get(task_id)

    def create(self, data: TaskCreate) -> Task:
        """Create a new task and return it."""
        now = datetime.now(UTC)
        task = Task(
            id=uuid4(),
            title=data.title,
            completed=False,
            created_at=now,
            updated_at=now,
        )
        self._tasks[task.id] = task
        return task

    def update(self, task_id: UUID, data: TaskUpdate) -> Task | None:
        """Update an existing task. Returns None if not found."""
        task = self._tasks.get(task_id)
        if task is None:
            return None

        update_data = data.model_dump(exclude_unset=True)
        if update_data:
            update_data["updated_at"] = datetime.now(UTC)
            updated_task = task.model_copy(update=update_data)
            self._tasks[task_id] = updated_task
            return updated_task
        return task

    def delete(self, task_id: UUID) -> bool:
        """Delete a task. Returns True if deleted, False if not found."""
        if task_id in self._tasks:
            del self._tasks[task_id]
            return True
        return False

    def clear(self) -> None:
        """Clear all tasks. Useful for testing."""
        self._tasks.clear()


# Global store instance
store = TaskStore()
