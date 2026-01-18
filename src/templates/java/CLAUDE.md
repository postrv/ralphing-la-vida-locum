# Project Memory - Java Automation Suite Enabled

## Environment
- Claude Code 2.1.0+ with skill hot-reload
- Opus 4.5 exclusively (Max x20)
- narsil-mcp for code intelligence and security
- Ralph automation suite for autonomous execution

## Current Work: Java Project

This is a Java project. Key conventions and tools:

- **Build**: Maven or Gradle
- **Testing**: JUnit 5 with assertions
- **Linting**: Checkstyle, SpotBugs
- **Security**: OWASP Dependency Check

---

## GIT AUTHENTICATION (CRITICAL)

### Required Setup
Ralph requires `gh` CLI for all GitHub operations. SSH key access is **blocked**.

```bash
# Verify gh CLI is authenticated
gh auth status

# If not authenticated, run:
gh auth login
```

### Rules - Non-Negotiable
1. **ALWAYS** use `gh` CLI for GitHub operations
2. **NEVER** attempt SSH key operations (ssh-keygen, ssh-add, etc.)
3. **NEVER** access ~/.ssh/ directory
4. **NEVER** use git@github.com: URLs
5. Use `gh auth status` to verify authentication
6. Use `gh repo clone` instead of `git clone git@github.com:`

---

## PRODUCTION STANDARDS (Non-Negotiable)

### Code Quality - Zero Tolerance Policy

**FORBIDDEN PATTERNS - Never Use:**
```java
@SuppressWarnings(...)      // Fix the warning properly
catch (Exception e) { }     // Never empty catch blocks
// TODO: ...                // Implement now or don't merge
// FIXME: ...               // Fix now
throw new RuntimeException("not implemented");  // Implement properly
```

**REQUIRED PATTERNS - Always Use:**
```java
/**
 * Short description of method.
 *
 * <p>Longer description if needed.
 *
 * @param input the input parameter description
 * @return the result description
 * @throws IllegalArgumentException if input is null or empty
 *
 * @example
 * <pre>{@code
 * Result result = methodName("input");
 * }</pre>
 */
public Result methodName(String input) {
    if (input == null || input.isEmpty()) {
        throw new IllegalArgumentException("input cannot be null or empty");
    }
    // ...
}
```

### Test-Driven Development (MANDATORY - NO EXCEPTIONS)

**Tests FIRST. Implementation SECOND. Always.**

**Every change follows this cycle:**
1. **REINDEX**: `reindex` - refresh narsil-mcp index before starting
2. **RED**: Write a failing JUnit test that defines expected behavior
3. **GREEN**: Write minimal code to make the test pass
4. **REFACTOR**: Clean up while keeping tests green
5. **REVIEW**: Run checkstyle + spotbugs + OWASP check
6. **COMMIT**: Only if ALL quality gates pass
7. **REINDEX**: `reindex` - refresh narsil-mcp index after completing

**If you write implementation before tests, STOP:**
1. Delete the implementation
2. Write the test first
3. Then write minimal code to pass

**Test Coverage Requirements:**
- Every public method: at least 1 unit test
- Every class: integration tests for key workflows
- Every exception path: tested explicitly
- Every edge case: null inputs, boundaries, empty collections

### Java Quality Tools

**Maven projects:**
```bash
# Run tests
mvn test

# Run checkstyle
mvn checkstyle:check

# Run SpotBugs
mvn spotbugs:check

# Run OWASP dependency check
mvn org.owasp:dependency-check-maven:check

# Full verification
mvn verify
```

**Gradle projects:**
```bash
# Run tests
./gradlew test

# Run checkstyle
./gradlew checkstyleMain checkstyleTest

# Run SpotBugs
./gradlew spotbugsMain

# Run OWASP dependency check
./gradlew dependencyCheckAnalyze

# Full verification
./gradlew build
```

---

## MCP SERVERS

### narsil-mcp (Code Intelligence - Optional)

narsil-mcp is optional. All features gracefully degrade when unavailable.

**Security (run before committing, if available):**
```bash
scan_security           # Find vulnerabilities
find_injection_vulnerabilities  # SQL/XSS/command injection
check_cwe_top25        # CWE Top 25 checks
```

**Context Gathering:**
```bash
get_call_graph <function>   # Method relationships
find_references <symbol>    # Impact analysis
get_dependencies <path>     # Package dependencies
find_similar_code <query>   # Find related code
```

### Graceful Degradation Policy

When narsil-mcp is unavailable:
- Security gates are skipped (log warning)
- Code intelligence returns empty results
- Ralph continues to function normally

---

## QUALITY GATES (Pre-Commit Checklist)

**Start of Task:**
```
[ ] reindex                                    -> narsil-mcp index refreshed
```

**Mandatory (always enforced):**
```
[ ] Tests written BEFORE implementation        -> TDD verified
[ ] mvn checkstyle:check                       -> 0 violations
[ ] mvn spotbugs:check                         -> 0 bugs
[ ] mvn test                                   -> all pass
[ ] OWASP dependency-check                     -> 0 CRITICAL/HIGH
[ ] No @SuppressWarnings without justification
[ ] No empty catch blocks
[ ] No TODO/FIXME comments in new code
[ ] All new public methods documented (Javadoc)
[ ] All new classes have tests
[ ] gh auth status                             -> authenticated
```

**Optional (if narsil-mcp available):**
```
[ ] scan_security                              -> 0 CRITICAL/HIGH
[ ] find_injection_vulnerabilities             -> 0 findings
```

**End of Task:**
```
[ ] reindex                                    -> narsil-mcp index updated
```

---

## JAVA-SPECIFIC CONVENTIONS

### Project Structure (Maven)
```
project/
├── src/
│   ├── main/
│   │   ├── java/
│   │   │   └── com/example/
│   │   │       ├── Application.java
│   │   │       ├── service/
│   │   │       └── repository/
│   │   └── resources/
│   └── test/
│       ├── java/
│       │   └── com/example/
│       │       └── service/
│       │           └── ServiceTest.java
│       └── resources/
├── pom.xml
└── README.md
```

### Imports
```java
// Java standard library first
import java.util.List;
import java.util.Optional;

// Third-party libraries second
import org.springframework.stereotype.Service;
import lombok.RequiredArgsConstructor;

// Project imports last
import com.example.repository.UserRepository;
import com.example.model.User;
```

### JUnit 5 Tests
```java
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.CsvSource;

import static org.junit.jupiter.api.Assertions.*;

class ServiceTest {

    private Service service;

    @BeforeEach
    void setUp() {
        service = new Service();
    }

    @Test
    @DisplayName("should handle basic case")
    void shouldHandleBasicCase() {
        // Given
        String input = "hello";

        // When
        Result result = service.process(input);

        // Then
        assertEquals(5, result.length());
    }

    @Test
    @DisplayName("should throw on null input")
    void shouldThrowOnNullInput() {
        assertThrows(IllegalArgumentException.class, () -> {
            service.process(null);
        });
    }

    @ParameterizedTest
    @CsvSource({
        "a, 1",
        "bb, 2",
        "ccc, 3"
    })
    @DisplayName("should return correct length for various inputs")
    void shouldReturnCorrectLength(String input, int expected) {
        Result result = service.process(input);
        assertEquals(expected, result.length());
    }
}
```

### Exception Handling
```java
// Always handle exceptions properly - never empty catch blocks
try {
    result = doSomething();
} catch (SpecificException e) {
    logger.error("Failed to do something: {}", e.getMessage(), e);
    throw new ServiceException("Operation failed", e);
}

// Use Optional instead of null
public Optional<User> findUser(String id) {
    return Optional.ofNullable(repository.findById(id));
}
```

---

## QUICK REFERENCE

**Maven:**
```bash
# Clean and build
mvn clean install

# Run tests
mvn test

# Run tests with verbose output
mvn test -X

# Skip tests (only for local builds)
mvn install -DskipTests

# Checkstyle
mvn checkstyle:check

# SpotBugs
mvn spotbugs:check

# OWASP check
mvn org.owasp:dependency-check-maven:check

# Verify git environment
gh auth status
```

**Gradle:**
```bash
# Clean and build
./gradlew clean build

# Run tests
./gradlew test

# Checkstyle
./gradlew checkstyleMain checkstyleTest

# SpotBugs
./gradlew spotbugsMain

# OWASP check
./gradlew dependencyCheckAnalyze

# Verify git environment
gh auth status
```
