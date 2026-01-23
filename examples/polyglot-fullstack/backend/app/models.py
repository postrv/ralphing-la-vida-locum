"""Pydantic models for the Task Manager API.

These models correspond to the OpenAPI schema in shared/openapi.yaml.
"""

from datetime import datetime
from uuid import UUID

from pydantic import BaseModel, ConfigDict, Field


class TaskCreate(BaseModel):
    """Request body for creating a new task."""

    title: str = Field(
        ...,
        min_length=1,
        max_length=200,
        description="The task title (required, 1-200 characters)",
    )


class TaskUpdate(BaseModel):
    """Request body for updating an existing task."""

    title: str | None = Field(
        default=None,
        min_length=1,
        max_length=200,
        description="New title for the task",
    )
    completed: bool | None = Field(
        default=None,
        description="New completion status",
    )


class Task(BaseModel):
    """A task item in the task manager."""

    model_config = ConfigDict(from_attributes=True)

    id: UUID = Field(..., description="Unique identifier for the task")
    title: str = Field(..., description="The task title/description")
    completed: bool = Field(default=False, description="Whether the task has been completed")
    created_at: datetime = Field(..., description="When the task was created")
    updated_at: datetime = Field(..., description="When the task was last updated")


class HealthResponse(BaseModel):
    """Response from the health check endpoint."""

    status: str = "healthy"
    version: str = "1.0.0"
