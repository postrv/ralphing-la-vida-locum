# Task Manager - Implementation Plan

> **Goal**: Extend the Task Manager with categories, due dates, and improved UX.
>
> **Methodology**: TDD across both frontend and backend. Each task follows RED -> GREEN -> REFACTOR.

---

## Sprint 1: Task Categories

### Phase 1.1: Backend - Category Model

**Test Requirements**:
- [ ] Test `Category` model has id, name, color fields
- [ ] Test category name validation (1-50 chars)
- [ ] Test color validation (hex format)
- [ ] Test task can have optional category_id

**Implementation**:
- [ ] Create `Category` Pydantic model
- [ ] Add `category_id` field to `Task` model
- [ ] Update `TaskCreate` and `TaskUpdate` schemas
- [ ] Update OpenAPI spec

**Quality Gates**:
```bash
cd backend && ruff check . && mypy app && pytest
```

### Phase 1.2: Backend - Category CRUD

**Test Requirements**:
- [ ] Test GET /api/categories returns empty list
- [ ] Test POST /api/categories creates category
- [ ] Test GET /api/categories/{id} returns category
- [ ] Test DELETE /api/categories/{id} removes category
- [ ] Test deleting category nullifies task category_id

**Implementation**:
- [ ] Create `CategoryStore` class
- [ ] Add category CRUD endpoints
- [ ] Handle cascading updates on category deletion

**Quality Gates**:
```bash
cd backend && ruff check . && mypy app && pytest
```

### Phase 1.3: Frontend - Category Display

**Test Requirements**:
- [ ] Test CategoryBadge component renders color
- [ ] Test CategoryBadge handles null category
- [ ] Test TaskList shows category badge

**Implementation**:
- [ ] Create `CategoryBadge` component
- [ ] Update `TaskList` to display categories
- [ ] Add category fetch to API client

**Quality Gates**:
```bash
cd frontend && npm run lint && npm run type-check
```

### Phase 1.4: Frontend - Category Selection

**Test Requirements**:
- [ ] Test CategorySelect renders options
- [ ] Test CategorySelect handles selection
- [ ] Test TaskForm includes category selection

**Implementation**:
- [ ] Create `CategorySelect` component
- [ ] Update `TaskForm` with category dropdown
- [ ] Update create/edit flows

**Quality Gates**:
```bash
cd frontend && npm run lint && npm run type-check
```

---

## Sprint 2: Due Dates

### Phase 2.1: Backend - Due Date Field

**Test Requirements**:
- [ ] Test `Task.due_date` is optional datetime
- [ ] Test due date validation (must be in future for new tasks)
- [ ] Test due date can be null
- [ ] Test filtering tasks by due date

**Implementation**:
- [ ] Add `due_date` field to Task model
- [ ] Add validation for future dates
- [ ] Add query parameter for filtering

**Quality Gates**:
```bash
cd backend && ruff check . && mypy app && pytest
```

### Phase 2.2: Frontend - Due Date Display

**Test Requirements**:
- [ ] Test DueDate component formats dates correctly
- [ ] Test DueDate shows "overdue" styling
- [ ] Test DueDate shows "due soon" styling
- [ ] Test TaskList sorts by due date option

**Implementation**:
- [ ] Create `DueDate` component
- [ ] Add due date formatting utility
- [ ] Update TaskList with due date display

**Quality Gates**:
```bash
cd frontend && npm run lint && npm run type-check
```

### Phase 2.3: Frontend - Due Date Input

**Test Requirements**:
- [ ] Test DatePicker component handles selection
- [ ] Test DatePicker validates future dates
- [ ] Test TaskForm includes date picker

**Implementation**:
- [ ] Create `DatePicker` component (or integrate library)
- [ ] Update `TaskForm` with due date input
- [ ] Update create/edit API calls

**Quality Gates**:
```bash
cd frontend && npm run lint && npm run type-check
```

---

## Sprint 3: UX Improvements

### Phase 3.1: Task Filtering

**Test Requirements**:
- [ ] Test filter by completion status
- [ ] Test filter by category
- [ ] Test filter by due date range
- [ ] Test filters combine correctly

**Implementation**:
- [ ] Add filter state to home page
- [ ] Create FilterBar component
- [ ] Update API client with query params

**Quality Gates**:
```bash
cd frontend && npm run lint && npm run type-check
```

### Phase 3.2: Task Search

**Test Requirements**:
- [ ] Test search by title substring
- [ ] Test search is case-insensitive
- [ ] Test search with filters combined

**Implementation**:
- [ ] Add search endpoint to backend
- [ ] Create SearchInput component
- [ ] Integrate with FilterBar

**Quality Gates**:
```bash
cd backend && ruff check . && mypy app && pytest
cd frontend && npm run lint && npm run type-check
```

### Phase 3.3: Drag and Drop Reordering

**Test Requirements**:
- [ ] Test tasks have position field
- [ ] Test reorder endpoint updates positions
- [ ] Test drag-drop UI updates order

**Implementation**:
- [ ] Add `position` field to Task model
- [ ] Add PATCH /api/tasks/reorder endpoint
- [ ] Integrate drag-drop library in frontend

**Quality Gates**:
```bash
cd backend && ruff check . && mypy app && pytest
cd frontend && npm run lint && npm run type-check
```

---

## Completion Criteria

When all sprints are complete:

1. All checkboxes marked
2. All quality gates passing
3. Both frontend and backend tests green
4. No lint warnings or type errors
5. OpenAPI spec updated to match implementation

---

## Notes for Claude

- **Run both language gates**: This is a polyglot project - check both frontend and backend
- **Update shared types**: When modifying API schemas, update both `openapi.yaml` and `types.ts`
- **Test isolation**: Backend tests use in-memory store that clears between tests
- **CORS**: Backend allows frontend origin - don't modify without testing integration
