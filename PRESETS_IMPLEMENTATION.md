# Presets Implementation Summary

This document summarizes the implementation of the presets feature for imgforge.

## Features Implemented

### 1. Preset Configuration

- **`IMGFORGE_PRESETS`** environment variable for defining named presets
  - Format: `name=options,name2=options2`
  - Example: `thumbnail=resize:fit:150:150/quality:80,banner=resize:fill:1200:300/quality:90`
  - Options within a preset are separated by `/` and follow standard processing option syntax

- **`IMGFORGE_ONLY_PRESETS`** environment variable for enabling presets-only mode
  - Default: `false`
  - When `true`, only `preset:name` (or `pr:name`) references are allowed in URLs
  - Other processing options return `400 Bad Request`
  - Useful for enforcing strict governance over allowed transformations

### 2. Default Preset

- A preset named `default` is automatically applied to every request
- Applied before any URL-specific options or named preset references
- URL options override default preset values when the same parameter appears in both
- Useful for setting organization-wide defaults (e.g., default quality, DPR)

### 3. Preset URL Syntax

- Full form: `preset:name`
- Short form: `pr:name`
- Can be used in URLs like any other processing option
- Example: `/signature/preset:thumbnail/encoded_url`
- Multiple presets can be chained: `/signature/preset:base/preset:quality_high/encoded_url`

### 4. Preset Expansion

Presets are expanded during request processing:
1. Default preset is applied (if it exists)
2. URL options/presets are processed left-to-right
3. Later options override earlier ones when conflicts occur

## Code Changes

### New Files

1. **`src/processing/presets.rs`** - Core preset expansion logic
   - `expand_presets()` - Expands preset references into processing options
   - `parse_options_string()` - Parses preset option strings
   - Unit tests for all preset functionality

2. **`tests/presets_integration_tests.rs`** - Integration tests
   - Tests for basic preset usage
   - Tests for default preset
   - Tests for presets-only mode
   - Tests for preset short forms
   - Tests for error handling

### Modified Files

1. **`src/constants.rs`** - Added new environment variable constants:
   - `ENV_PRESETS`
   - `ENV_ONLY_PRESETS`

2. **`src/config.rs`** - Extended Config struct:
   - Added `presets: HashMap<String, String>` field
   - Added `only_presets: bool` field
   - Added `parse_presets()` function
   - Added unit tests for preset parsing

3. **`src/processing/mod.rs`** - Added presets module to exports

4. **`src/handlers.rs`** - Integrated preset expansion:
   - Call `expand_presets()` before `parse_all_options()`
   - Applied to both cached and non-cached request paths

5. **`tests/handlers_integration_tests.rs`** - Updated test helpers:
   - Added `presets` and `only_presets` fields to `create_test_config()`

6. **`tests/handlers_integration_tests_extended.rs`** - Updated test helpers:
   - Added `presets` and `only_presets` fields to `create_test_config()`

### Documentation Updates

1. **`doc/3_configuration.md`** - Added Presets section:
   - Description of preset configuration
   - Usage examples
   - Explanation of default preset and presets-only mode

2. **`doc/5_processing_options.md`** - Added preset option:
   - Added `preset` to quick reference table
   - Added dedicated Presets section with examples
   - Explained preset expansion behavior

3. **`README.md`** - Added presets to feature highlights

## Testing

### Unit Tests (24 tests)

**Config module (12 tests):**
- Preset parsing (single, multiple, with spaces, empty, invalid formats)
- Config loading from environment variables
- Default behavior for `only_presets`

**Presets module (12 tests):**
- Options string parsing
- Preset expansion (simple, with default, chaining)
- Unknown preset error handling
- Presets-only mode validation
- Short form preset references

### Integration Tests (9 tests)

- Basic preset usage with image transformation
- Default preset automatic application
- Default preset with additional URL options
- Unknown preset error handling
- Presets-only mode allows presets
- Presets-only mode rejects non-presets
- Presets-only mode allows default preset
- Short form preset references (`pr:name`)
- URL options override preset values

## Usage Examples

### Basic Configuration

```bash
export IMGFORGE_PRESETS="thumbnail=resize:fit:150:150/quality:80,avatar=resize:fill:64:64/quality:85"
export IMGFORGE_KEY=<your-key>
export IMGFORGE_SALT=<your-salt>
```

### With Default Preset

```bash
export IMGFORGE_PRESETS="default=quality:90/dpr:1,thumbnail=resize:fit:150:150"
```

### Presets-Only Mode

```bash
export IMGFORGE_PRESETS="small=resize:fit:300:300,medium=resize:fit:600:600,large=resize:fit:1200:1200"
export IMGFORGE_ONLY_PRESETS="true"
```

### URL Examples

```
# Using a named preset
/signature/preset:thumbnail/encoded_url

# Using short form
/signature/pr:avatar/encoded_url

# Chaining presets
/signature/preset:base/preset:high_quality/encoded_url

# With default preset (applied automatically)
/signature/encoded_url

# Override preset values
/signature/preset:thumbnail/quality:95/encoded_url
```

## Benefits

1. **Consistency** - Define transformation sets once, use everywhere
2. **Governance** - Control allowed transformations via server configuration
3. **Simplicity** - Shorter, more readable URLs
4. **Maintainability** - Change transformations globally by updating preset definitions
5. **Security** - Presets-only mode prevents arbitrary transformation requests
6. **Flexibility** - URL options can still override preset values when needed

## Backward Compatibility

- All existing URLs continue to work unchanged
- Presets are opt-in via configuration
- No breaking changes to existing functionality
