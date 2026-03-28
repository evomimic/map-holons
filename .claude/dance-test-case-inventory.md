# Dance Test Case Inventory

This document captures the **step-by-step functionality and edge cases** tested by each
integration test case in `tests/sweetests/tests/dance_tests.rs`, to serve as a regression
checklist during test rewrites.

---

## Shared Setup Helper: `setup_book_author_steps_with_context`

**File:** `tests/sweetests/tests/fixture_cases/setup_book_and_authors_fixture.rs`

Used by: `simple_abandon_staged_changes`, `simple_add_remove_properties`,
`simple_add_remove_related_holons`, `stage_new_from_clone`, `stage_new_version`

Stages 4 holons **without committing**:

| Holon       | Key            | Properties                                                    |
|-------------|----------------|---------------------------------------------------------------|
| Book        | `BOOK_KEY`     | Title = BOOK_KEY, Description = (long string)                 |
| Person 1    | `PERSON_1_KEY` | first name = "Roger", last name = "Briggs"                    |
| Person 2    | `PERSON_2_KEY` | first name = "George", last name = "Smith"                    |
| Publisher   | `PUBLISHER_KEY`| name = PUBLISHER_KEY, Description = "We publish Holons..."    |

Relationships added:
- `Book → BOOK_TO_PERSON_RELATIONSHIP → [Person1, Person2]`

All holons go through **new_holon → add_new_holon_step → stage_holon_step**.
Tokens for Book, Person1, Person2, Publisher, and the relationship name are stored in `FixtureBindings`.

---

## 1. `simple_undescribed_create_holon_test` → `simple_create_holon_fixture()`

**File:** `fixture_cases/simple_create_holon_fixture.rs`

**Purpose:** Validates the minimal create-stage-commit-verify lifecycle for a single holon.

### Steps

| # | Step                        | Detail                                                                         | Expected |
|---|-----------------------------|--------------------------------------------------------------------------------|----------|
| 1 | Ensure DB Count             | Assert DB has 0 saved holons (labeled "ENSURE_DB_EMPTY")                       | OK       |
| 2 | NewHolon                    | Create book transient with title=BOOK_KEY, description=(long string)           | OK       |
| 3 | StageHolon                  | Stage the book                                                                  | OK       |
| 4 | Commit                      | Commit all staged holons                                                        | OK       |
| 5 | Ensure DB Count             | Assert saved count matches fixture expectation (1 holon)                       | OK       |
| 6 | MatchSavedContent           | Assert all saved holons match their fixture snapshots                           | OK       |

### Functionality Covered
- Single holon lifecycle: transient → staged → committed → saved
- Property setting at creation time
- Database count assertion (empty start + post-commit)
- Content match of saved holons against expected snapshots

---

## 2. `delete_holon` → `delete_holon_fixture()`

**File:** `fixture_cases/delete_holon_fixture.rs`

**Purpose:** Tests the delete_holon dance, including both successful deletion and attempting to delete an already-deleted holon.

### Steps

| # | Step                        | Detail                                                                         | Expected        |
|---|-----------------------------|--------------------------------------------------------------------------------|-----------------|
| 1 | NewHolon                    | Create book transient with Title=BOOK_KEY, description=(long string)           | OK              |
| 2 | StageHolon                  | Stage the book                                                                  | OK              |
| 3 | Commit                      | Commit all staged holons                                                        | OK              |
| 4 | Ensure DB Count             | Assert DB count matches (1 holon)                                              | OK              |
| 5 | DeleteHolon (valid)         | Delete the committed book holon                                                 | OK              |
| 6 | DeleteHolon (invalid)       | Attempt to delete the same (already deleted) holon again                        | **NotFound**    |

### Functionality Covered
- Successful deletion of a committed holon
- **Negative test:** double-delete returns `NotFound`
- Note: DB count assertion after delete is commented out (TODO: link cleanup needed)

### Edge Cases
- Reuse of the same `TestReference` for the second delete — tests that the framework resolves the already-deleted holon correctly

---

## 3. `simple_abandon_staged_changes_test` → `simple_abandon_staged_changes_fixture()`

**File:** `fixture_cases/abandon_staged_changes_fixture.rs`

**Purpose:** Tests the abandon_staged_changes dance across multiple scenarios — abandoning a holon from the shared setup, then creating and abandoning additional holons, and verifying commit behavior after abandons.

### Steps

| #  | Step                        | Detail                                                                         | Expected |
|----|-----------------------------|--------------------------------------------------------------------------------|----------|
| 1  | Ensure DB Count             | Assert DB is empty                                                             | OK       |
| 2–9| Setup (shared helper)       | Stage Book, Person1, Person2, Publisher + AUTHORED_BY relationships            | OK       |
| 10 | AbandonStagedChanges        | Abandon Person1 (from setup)                                                   | OK       |
| 11 | Commit                      | Commit remaining staged holons (Book, Person2, Publisher survive)              | OK       |
| 12 | Ensure DB Count             | Assert saved count matches (3 holons — Person1 was abandoned)                  | OK       |
| 13 | MatchSavedContent           | Assert saved holons match snapshots                                            | OK       |
| 14 | NewHolon (Abandon1)         | Create transient holon with key="Abandon1", property "example abandon1"="test1"| OK       |
| 15 | StageHolon (Abandon1)       | Stage Abandon1                                                                  | OK       |
| 16 | NewHolon (Abandon2)         | Create transient holon with key="Abandon2", property "example abandon2"="test2"| OK       |
| 17 | StageHolon (Abandon2)       | Stage Abandon2                                                                  | OK       |
| 18 | AbandonStagedChanges        | Abandon Abandon1 (H4)                                                           | OK       |
| 19 | AbandonStagedChanges        | Abandon Abandon2 (H5)                                                           | OK       |
| 20 | Commit                      | Commit (nothing should be staged — both were abandoned)                        | OK       |
| 21 | Ensure DB Count             | Assert saved count unchanged (still 3)                                         | OK       |
| 22 | MatchSavedContent           | Assert saved holons still match                                                | OK       |

### Functionality Covered
- Abandoning a holon that is part of a relationship (Person1 was target of Book's AUTHORED_BY)
- Commit after abandon correctly excludes abandoned holons
- Abandoning freshly created holons (not part of any relationship)
- Abandoning multiple holons in sequence
- Commit with no remaining staged holons succeeds
- Database integrity maintained through abandon + commit cycles

### Edge Cases
- Commented-out step: attempt to add a relationship to an abandoned holon (expected Conflict/NotAccessible) — not yet implemented
- Commented-out step: query relationships on book after Person1 abandoned — not yet active

---

## 4. `simple_add_remove_properties_test` → `simple_add_remove_properties_fixture()`

**File:** `fixture_cases/simple_add_remove_properties_fixture.rs`

**Purpose:** Tests adding and removing properties on both **Transient** and **Staged** holons.

### Steps

| #  | Step                        | Detail                                                                         | Expected |
|----|-----------------------------|--------------------------------------------------------------------------------|----------|
| 1–9| Setup (shared helper)       | Stage Book, Person1, Person2, Publisher + relationships                        | OK       |
| 10 | NewHolon (Example)          | Create transient holon with key="EXAMPLE_KEY" (no initial properties)          | OK       |
| 11 | WithProperties (Transient)  | Add 4 properties to Example: Description(string), ExampleProperty(string), Integer(-1), Boolean(false) | OK |
| 12 | WithProperties (Staged)     | Add 5 properties to Book: Description("Changed description"), Title(BOOK_KEY), NewProperty(string), Int(42), Bool(true) | OK |
| 13 | RemoveProperties (Transient)| Remove Integer and Boolean from Example                                        | OK       |
| 14 | RemoveProperties (Staged)   | Remove NewProperty, Int, and Bool from Book                                    | OK       |

### Functionality Covered
- Adding multiple properties of different types: string, integer, boolean
- Adding properties to a **transient** holon (not yet staged)
- Adding properties to a **staged** holon (already in nursery)
- Removing properties from a transient holon
- Removing properties from a staged holon
- Note: property removal only uses property names; values in the removal map are ignored

### Edge Cases / TODOs
- TODO: removing a property that doesn't exist
- TODO: adding an invalid property
- TODO: re-adding properties after removal (step commented as planned but not implemented)

---

## 5. `simple_add_related_holon_test` → `simple_add_remove_related_holons_fixture()`

**File:** `fixture_cases/simple_add_remove_related_holons_fixture.rs`

**Purpose:** Tests adding and removing related holons (relationships) on both **Transient** and **Staged** references, plus commit and query.

### Steps

| #  | Step                           | Detail                                                                      | Expected |
|----|--------------------------------|-----------------------------------------------------------------------------|----------|
| 1  | Ensure DB Count                | Assert DB is "empty" (only initial LocalHolonSpace)                         | OK       |
| 2–9| Setup (shared helper)          | Stage Book, Person1, Person2, Publisher + AUTHORED_BY                       | OK       |
| 10 | NewHolon (Company)             | Create transient Company with name="The Really Useful Information Company"  | OK       |
| 11 | NewHolon (Website)             | Create transient Website with url="itsyourworld.com"                        | OK       |
| 12 | AddRelatedHolons (Transient)   | Company → HOST → [Website]                                                  | OK       |
| 13 | RemoveRelatedHolons (Transient)| Company → HOST → [Website] (remove)                                         | OK       |
| 14 | NewHolon (Example)             | Create transient Example with example="Example Holon"                       | OK       |
| 15 | AddRelatedHolons (Transient)   | Company → AGAIN → [Example] (re-add with different relationship)            | OK       |
| 16 | RemoveRelatedHolons (Staged)   | Book → AUTHORED_BY → [Person1] (remove one of two targets)                  | OK       |
| 17 | AddRelatedHolons (Staged)      | Book → PUBLISHED_BY → [Publisher]                                            | OK       |
| 18 | Commit                         | Commit all staged holons                                                     | OK       |
| 19 | Ensure DB Count                | Assert saved count matches                                                   | OK       |
| 20 | QueryRelationships             | Query Book's BOOK_TO_PERSON_RELATIONSHIP — expect Person2 only              | OK       |

### Functionality Covered
- Adding a relationship on a **transient** holon
- Removing a relationship on a **transient** holon
- Re-adding a different relationship after removal (transient)
- Removing one target from a multi-target relationship on a **staged** holon
- Adding a new relationship to a staged holon
- Commit after relationship modifications
- **QueryRelationships** — verifying relationship state post-commit (Person1 removed, Person2 remains)

### Edge Cases / TODOs
- TODO: removing related holons using invalid source and relationship name

---

## 6. `ergonomic_add_remove_properties_test` → `ergonomic_add_remove_properties_fixture()`

**File:** `fixture_cases/ergonomic_add_remove_properties_fixture.rs`

**Purpose:** Tests all combinations of the `ToPropertyName` and `ToBaseValue` ergonomic traits for property operations. This fixture uses **direct API calls** (not harness execution steps) and inline assertions.

### Transient Phase

| Operation | PropertyName Type | BaseValue Type | Example                                        |
|-----------|-------------------|----------------|-------------------------------------------------|
| Add       | Enum              | str            | `Description`, `"Changed description"`          |
| Add       | String            | String         | `"NewProperty".to_string()`, `"...".to_string()`|
| Add       | str               | int            | `"Int"`, `42`                                   |
| Add       | str               | bool           | `"Bool"`, `true`                                |
| Remove    | String            | —              | `"NewProperty".to_string()`                     |
| Remove    | str               | —              | `"Int"`                                         |
| Remove    | MapString         | —              | `MapString("Bool".to_string())`                 |

**Assert:** `EssentialHolonContent` matches after add, and again after remove.

### Staged Phase

| Operation | PropertyName Type  | BaseValue Type | Example                                         |
|-----------|--------------------|----------------|-------------------------------------------------|
| Add       | PropertyName       | String         | `PropertyName(MapString("Description"...))`, `"Another...".to_string()` |
| Add       | MapString          | MapString      | `MapString("AnotherProperty"...)`, `MapString("Adding..."...)` |
| Remove    | Enum               | —              | `Description`                                    |
| Remove    | PropertyName       | —              | `PropertyName(MapString("AnotherProperty"...))`  |

**Assert:** `EssentialHolonContent` matches after add, and again after remove.

### Functionality Covered
- Exhaustive trait dispatch coverage for `ToPropertyName`: Enum, String, str, MapString, PropertyName
- Exhaustive trait dispatch coverage for `ToBaseValue`: String, str, MapString, int, bool, BaseValue
- Direct API (`with_property_value`, `remove_property_value`) on both TransientReference and StagedReference
- Chained method calls (builder pattern)
- `EssentialHolonContent` structural equality

### Notable
- Does **not** use execution steps — operates directly on `WritableHolon` functions
- No commit; purely tests the in-memory property API

---

## 7. `ergonomic_add_remove_related_holons_test` → `ergonomic_add_remove_related_holons_fixture()`

**File:** `fixture_cases/ergonomic_add_remove_related_holons_fixture.rs`

**Purpose:** Tests all combinations of the `ToRelationshipName` ergonomic trait for relationship operations. Uses **direct API calls** and inline assertions.

### Transient Phase

| Operation | RelationshipName Type | Example                                   |
|-----------|-----------------------|-------------------------------------------|
| Add       | str                   | `PUBLISHED_BY` → [Publisher]              |
| Add       | String                | `BOOK_TO_PERSON_RELATIONSHIP.to_string()` → [Person1, Person2] |
| Add       | Enum                  | `DescribedBy` → [Descriptor]             |
| Remove    | str                   | `PUBLISHED_BY` → [Publisher]              |
| Remove    | String                | `BOOK_TO_PERSON_RELATIONSHIP.to_string()` → [Person1] |
| Remove    | Enum                  | `DescribedBy` → [Descriptor]             |

**Assert:** `EssentialRelationshipMap` matches after add, and again after remove.

### Staged Phase

| Operation | RelationshipName Type | Example                                   |
|-----------|-----------------------|-------------------------------------------|
| Add       | MapString             | `EDITOR_FOR` → [Publisher (staged)]       |
| Add       | RelationshipName      | `"DescribedBy"` → [Descriptor (staged)]  |
| Remove    | MapString             | `EDITOR_FOR` → [Publisher (staged)]       |
| Remove    | RelationshipName      | `"DescribedBy"` → [Descriptor (staged)]  |

**Assert:** `EssentialRelationshipMap` matches after add, and again after remove.

### Functionality Covered
- Exhaustive trait dispatch for `ToRelationshipName`: str, String, Enum, MapString, RelationshipName
- Adding multiple targets in a single call (`add_related_holons` with vec of 2)
- Removing a single target from a multi-target relationship (Person1 from BOOK_TO_PERSON, leaving Person2)
- Both TransientReference and StagedReference relationship operations
- Chained method calls
- `EssentialRelationshipMap` structural equality

### Notable
- Does **not** use execution steps — operates directly on `WritableHolon` functions
- No commit; purely tests the in-memory relationship API

---

## 8. `stage_new_from_clone_test` → `stage_new_from_clone_fixture()`

**File:** `fixture_cases/stage_new_from_clone_fixture.rs`

**Purpose:** Tests cloning holons from three different reference states: transient (error), staged (success), and saved (success).

### Steps

| #  | Step                           | Detail                                                                      | Expected        |
|----|--------------------------------|-----------------------------------------------------------------------------|-----------------|
| 1  | Ensure DB Count                | Assert DB starts with 1 (space holon)                                       | OK              |
| **Phase A: Clone from Transient** |                                                                  |                 |
| 2  | NewHolon                       | Create transient "book:transient-source"                                    | OK              |
| 3  | StageNewFromClone              | Attempt to clone from the transient (not yet staged)                        | **BadRequest**  |
| **Phase B: Clone from Staged** |                                                                     |                 |
| 4–11| Setup (shared helper)         | Stage Book, Person1, Person2, Publisher + relationships                     | OK              |
| 12 | StageNewFromClone              | Clone from staged Book → new key "book:clone:from-staged"                   | OK              |
| 13 | WithProperties                 | Add properties: Description, TITLE="Dune", EDITION=2                        | OK              |
| 14 | Commit (Round 1)               | Commit all staged holons                                                     | OK              |
| 15 | Ensure DB Count                | Assert count after Round 1                                                   | OK              |
| **Phase C: Clone from Saved** |                                                                      |                 |
| 16 | StageNewFromClone              | Clone from saved Book (same token, now resolved as Saved) → "book:clone:from-saved" | OK      |
| 17 | WithProperties                 | Add properties: Description, TITLE="Saved Clone of Dune", EDITION=3, TYPE="Book Clone" | OK |
| 18 | Commit (Round 2)               | Commit the new clone                                                         | OK              |
| 19 | Ensure DB Count                | Assert count after Round 2                                                   | OK              |
| 20 | MatchSavedContent              | Assert all saved holons match fixture snapshots                              | OK              |

### Functionality Covered
- **Negative test:** cloning from a transient reference returns `BadRequest`
- Cloning from a **staged** reference produces a new staged holon
- Cloning from a **saved** reference (using same token that was staged, then committed) produces a new staged holon
- Adding properties to a cloned holon
- Multi-commit lifecycle (two rounds of commit)
- Database count and content verification across multiple commit rounds

### Edge Cases
- The same `book_staged_token` is reused across phases B and C — after commit it resolves as Saved
- Clone gets a new key distinct from the original

---

## 9. `stage_new_version_test` → `stage_new_version_fixture()`

**File:** `fixture_cases/stage_new_version_fixture.rs`

**Purpose:** Tests the stage_new_version dance, creating multiple versions of the same holon and detecting duplicate-key conflicts.

### Steps

| #  | Step                           | Detail                                                                      | Expected        |
|----|--------------------------------|-----------------------------------------------------------------------------|-----------------|
| 1–9| Setup (shared helper)         | Stage Book, Person1, Person2, Publisher + relationships                     | OK              |
| 10 | Ensure DB Count                | Assert DB starts "empty"                                                    | OK              |
| 11 | Commit                         | Commit setup holons                                                          | OK              |
| 12 | Ensure DB Count                | Assert count after commit                                                    | OK              |
| 13 | MatchSavedContent              | Assert saved holons match                                                    | OK              |
| 14 | StageNewVersion                | Stage first version of Book (version_count=1)                               | OK              |
| 15 | WithProperties                 | Set Description="This is a different description", Title="Changed"          | OK              |
| 16 | Commit                         | Commit first version                                                         | OK              |
| 17 | Ensure DB Count                | Assert count                                                                 | OK              |
| 18 | MatchSavedContent              | Assert saved content                                                         | OK              |
| 19 | StageNewVersion                | Stage second version of Book (version_count=1, no expected failure)          | OK              |
| 20 | StageNewVersion                | Stage third version of Book (version_count=2, expected_failure_code=Conflict)| OK (step succeeds but `get_staged_holon_by_base_key` returns **Conflict**) |

### Functionality Covered
- Creating a new version from a saved/committed holon
- Modifying properties on a versioned holon
- Multi-version lifecycle with intermediate commits
- **Conflict detection:** when multiple staged holons share the same base key, `get_staged_holon_by_base_key` returns Conflict
- Version counter tracking (version_count parameter)

### Edge Cases
- Third version (step 20) is expected to succeed as a step but internally triggers a `Conflict` from `get_staged_holon_by_base_key` because there are now >1 staged holons with the same key
- TODO: add/remove relationships on versioned holons

---

## 10. `load_holons_test` → `loader_incremental_fixture()`

**File:** `fixture_cases/load_holons_fixture.rs`

**Purpose:** Exercises the holon loader's two-pass workflow incrementally across 7 scenarios, building up the database count step by step.

### Steps

| #  | Scenario                        | Bundle Contents                                             | Expected Status | DB Δ | Staged | Committed | Links | Errors |
|----|---------------------------------|-------------------------------------------------------------|-----------------|------|--------|-----------|-------|--------|
| 1  | Ensure DB Count                 | — (assert DB = 1, space holon only)                         | —               | —    | —      | —         | —     | —      |
| 2  | **Empty bundle**                | 1 bundle, 0 members                                        | OK*             | 0    | 0      | 0         | 0     | 0      |
| 3  | Ensure DB Count = 1             |                                                             |                 |      |        |           |       |        |
| 4  | **Nodes-only**                  | 3 LoaderHolons (Book, Person, Publisher), no relationships  | OK              | +3   | 3      | 3         | 0     | 0      |
| 5  | Ensure DB Count = 4             |                                                             |                 |      |        |           |       |        |
| 6  | **Declared relationship**       | 2 LoaderHolons (Book, Person1) + 1 declared AUTHORED_BY LRR| OK              | +2   | 2      | 2         | 1     | 0      |
| 7  | Ensure DB Count = 6             |                                                             |                 |      |        |           |       |        |
| 8  | **Inverse + inline micro-schema** | 6 LoaderHolons (2 instances + 4 schema descriptors) + inverse LRR | OK      | +6   | 6      | 6         | 6     | 0      |
| 9  | Ensure DB Count = 12            |                                                             |                 |      |        |           |       |        |
| 10 | **Multi-bundle happy path**     | Bundle F1: Book node; Bundle F2: Person + declared link to Book | OK          | +2   | 2      | 2         | 1     | 0      |
| 11 | Ensure DB Count = 14            |                                                             |                 |      |        |           |       |        |
| 12 | **Multi-bundle duplicate-key**  | 2 bundles each with same LoaderHolon key, different offsets | OK*             | 0    | 2      | 0         | 0     | 1      |
| 13 | Ensure DB Count = 14            |                                                             |                 |      |        |           |       |        |

*Empty bundle and duplicate-key scenarios return OK at the step level but with specific metric assertions.

### Functionality Covered
- **Empty bundle short-circuit:** loader handles a bundle with no members gracefully
- **Nodes-only loading:** Pass-1 staging + commit without any relationships
- **Declared relationship resolution:** Forward-direction LRR creates one link
- **Inverse relationship resolution with inline micro-schema:**
  - Inline type descriptors (BookType, PersonType, DeclaredRelType, InverseRelType)
  - Schema links: SourceType, TargetType, InverseOf
  - Instance typing: DescribedBy links
  - Inverse LRR (Person AUTHORS Book) mapped to declared (Book AUTHORED_BY Person)
  - Total: 6 nodes, 6 links (3 schema + 2 DescribedBy + 1 declared)
- **Cross-bundle resolution:** Target holon key lives in a different bundle than the source
- **Duplicate-key detection:** Same key in two bundles → error, commit skipped, DB unchanged
  - Error holons enriched with LoaderHolonKey, Filename, StartUtf8ByteOffset provenance

### Edge Cases
- DB count is verified after **every** load step to ensure no side effects
- Duplicate-key: Pass-1 still stages both holons, but commit is skipped (staged=2, committed=0)

---

## 11. `load_holons_client_test` → `loader_client_fixture()`

**File:** `fixture_cases/loader_client_fixture.rs`

**Purpose:** End-to-end test of the loader client entrypoint using real MAP core schema JSON files.

### Steps

| # | Step                    | Detail                                                                         | Expected |
|---|-------------------------|--------------------------------------------------------------------------------|----------|
| 1 | LoadHolonsClient        | Load 7 core schema JSON files via the loader_client entrypoint                 | OK       |

### Assertions

| Metric             | Expected Value |
|--------------------|----------------|
| Holons staged      | 182            |
| Holons committed   | 182            |
| Links created      | 1,060          |
| Errors             | 0              |
| Total bundles      | 7              |
| Total loader holons| 182            |

### Files Loaded
1. `MAP Schema Types-map-core-schema-abstract-value-types.json`
2. `MAP Schema Types-map-core-schema-concrete-value-types.json`
3. `MAP Schema Types-map-core-schema-dance-schema.json`
4. `MAP Schema Types-map-core-schema-keyrules-schema.json`
5. `MAP Schema Types-map-core-schema-property-types.json`
6. `MAP Schema Types-map-core-schema-relationship-types.json`
7. `MAP Schema Types-map-core-schema-root.json`

### Functionality Covered
- Full loader-client pipeline: JSON parsing → bundle construction → two-pass loading → commit
- Bootstrap import schema (`BOOTSTRAP_IMPORT_SCHEMA_PATH`) parsing
- Large-scale loading: 182 holons, 1060 links across 7 bundles
- Real MAP core schema data (not synthetic test data)

---

## Summary: Step Type Coverage Across All Tests

| Step Type              | Tests Using It                                                          |
|------------------------|-------------------------------------------------------------------------|
| NewHolon               | 1, 2, 3, 4, 5                                                         |
| StageHolon             | 1, 2, 3, 4, 5                                                         |
| WithProperties         | 4, 8, 9                                                                |
| RemoveProperties       | 4                                                                       |
| AddRelatedHolons       | 5 (transient + staged)                                                 |
| RemoveRelatedHolons    | 5 (transient + staged)                                                 |
| Commit                 | 1, 2, 3, 5, 8, 9                                                      |
| DeleteHolon            | 2                                                                       |
| AbandonStagedChanges   | 3                                                                       |
| StageNewFromClone      | 8                                                                       |
| StageNewVersion        | 9                                                                       |
| EnsureDatabaseCount    | 1, 2, 3, 5, 8, 9, 10                                                  |
| MatchSavedContent      | 1, 3, 8, 9                                                             |
| QueryRelationships     | 5                                                                       |
| LoadHolons             | 10                                                                      |
| LoadHolonsClient       | 11                                                                      |
| Ergonomic (direct API) | 6 (properties), 7 (relationships)                                      |

## Negative / Error Test Coverage

| Scenario                                     | Test | Expected Code     |
|----------------------------------------------|------|-------------------|
| Delete already-deleted holon                  | 2    | NotFound          |
| Clone from transient (not staged)             | 8    | BadRequest        |
| Duplicate base key in staging (>1 version)    | 9    | Conflict          |
| Empty loader bundle                           | 10   | 0 staged/committed|
| Duplicate LoaderHolon key across bundles      | 10   | 1 error, 0 committed |
