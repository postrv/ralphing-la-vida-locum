"""FastAPI application entry point."""

from uuid import UUID

from fastapi import FastAPI, HTTPException, status
from fastapi.middleware.cors import CORSMiddleware

from app.models import HealthResponse, Task, TaskCreate, TaskUpdate
from app.store import store

app = FastAPI(
    title="Task Manager API",
    description="A simple task management API demonstrating Ralph's polyglot features.",
    version="1.0.0",
)

# Configure CORS for frontend access
app.add_middleware(
    CORSMiddleware,
    allow_origins=["http://localhost:3000"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)


@app.get("/api/health", response_model=HealthResponse, tags=["System"])
async def health_check() -> HealthResponse:
    """Health check endpoint."""
    return HealthResponse()


@app.get("/api/tasks", response_model=list[Task], tags=["Tasks"])
async def list_tasks() -> list[Task]:
    """List all tasks."""
    return store.list_all()


@app.post(
    "/api/tasks",
    response_model=Task,
    status_code=status.HTTP_201_CREATED,
    tags=["Tasks"],
)
async def create_task(data: TaskCreate) -> Task:
    """Create a new task."""
    return store.create(data)


@app.get("/api/tasks/{task_id}", response_model=Task, tags=["Tasks"])
async def get_task(task_id: UUID) -> Task:
    """Get a specific task by ID."""
    task = store.get(task_id)
    if task is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="Task not found",
        )
    return task


@app.patch("/api/tasks/{task_id}", response_model=Task, tags=["Tasks"])
async def update_task(task_id: UUID, data: TaskUpdate) -> Task:
    """Update an existing task."""
    task = store.update(task_id, data)
    if task is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="Task not found",
        )
    return task


@app.delete(
    "/api/tasks/{task_id}",
    status_code=status.HTTP_204_NO_CONTENT,
    tags=["Tasks"],
)
async def delete_task(task_id: UUID) -> None:
    """Delete a task."""
    if not store.delete(task_id):
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="Task not found",
        )
