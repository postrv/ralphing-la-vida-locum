/**
 * Shared TypeScript types for the Task Manager API.
 *
 * These types match the OpenAPI schema defined in openapi.yaml and are
 * used by both the frontend and can be generated for any language.
 */

/**
 * A task item in the task manager.
 */
export interface Task {
  /** Unique identifier for the task */
  id: string;
  /** The task title/description */
  title: string;
  /** Whether the task has been completed */
  completed: boolean;
  /** ISO 8601 timestamp when the task was created */
  created_at: string;
  /** ISO 8601 timestamp when the task was last updated */
  updated_at: string;
}

/**
 * Request body for creating a new task.
 */
export interface TaskCreate {
  /** The task title (required, 1-200 characters) */
  title: string;
}

/**
 * Request body for updating an existing task.
 * All fields are optional - only provided fields will be updated.
 */
export interface TaskUpdate {
  /** New title for the task */
  title?: string;
  /** New completion status */
  completed?: boolean;
}

/**
 * Standard error response from the API.
 */
export interface ApiError {
  /** Human-readable error message */
  detail: string;
}
