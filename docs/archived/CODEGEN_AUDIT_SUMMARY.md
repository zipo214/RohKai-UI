# RohKai Codegen Audit — Executive Summary

## Audit Scope
Comprehensive security and correctness review of the codegen pipeline:
- `src/codegen/egui_emitter.rs` — Widget → Rust code emission
- `src/codegen/state_emitter.rs` — AppState struct generation
- `src/codegen/parser.rs` — Lazare bidirectional sync (Rust → canvas)
- `src/codegen/export.rs` — Complete project export
- `src/codegen/rust*.rs` — Helper utilities

**Duration:** Full codebase analysis + edge case tracing

---

## Key Findings: 9 Bugs Identified

| # | Severity | Category | Issue | Impact |
|---|----------|----------|-------|--------|
| 1 | **HIGH** | Correctness | Unwrap panic in parser tests | Crash → silent data loss |
| 2 | **HIGH** | Bidirectional | Silent parse failure → data loss | Canvas/code divergence |
| 3 | **MEDIUM** | Correctness | Invalid string escaping (surrogates) | Compile failure on Unicode |
| 4 | **MEDIUM** | Correctness | Channel name sanitization incomplete | Compile failure on digits |
| 5 | **MEDIUM** | Correctness | Export test file missing panic | Unclear test failure |
| 6 | **HIGH** | Bidirectional | Malformed binding silent drop | Canvas desync without error |
| 7 | **MEDIUM** | Bidirectional | Invalid identifier from parser | Compile failure after user edit |
| 8 | **MEDIUM** | State | Keyword collision undetected | Generated code invalid |
| 9 | **MEDIUM** | Edge Case | Deep nesting unreadable indent | Maintainability issue |

---

## Critical Issues (Data Loss / Silent Failure)

### Issue #1 & #2: Unwraps in Parser (HIGH)
**Location:** `parser.rs:667-668, 782-784, 815`
**Problem:** Test code uses `.unwrap()` on parser results without checking success
**Risk:** If parser fails, panics instead of reporting error
**Fix:** Use `.expect("descriptive message")` or proper error handling

### Issue #6: Malformed Binding Silent Drop (HIGH)
**Location:** `parser.rs:479-492` (extract_binding_name)
**Problem:** Binding like `&mut self.` (missing identifier) is silently dropped
**Risk:** User edits code, parser fails silently, canvas and code diverge
**Fix:** Detect malformed bindings and report error diagnostic

---

## Compilation Failures

### Issue #3: String Escaping (MEDIUM)
**Problem:** Unicode surrogates not escaped by `format!("{:?}")`
**Example:** Label with lone surrogate `\u{D800}` → invalid Rust literal
**Fix:** Use `escape_default()` as fallback for invalid Unicode

### Issue #4: Channel Name Sanitization (MEDIUM)
**Problem:** `sanitize()` produces `"123abc"` (digits first = invalid identifier)
**Example:** Channel named `"123data"` → generated `123data_tx` ← invalid Rust
**Fix:** Prepend underscore if first char is digit

### Issue #7: Identifier Validation (MEDIUM)
**Problem:** Parser extracts identifiers without validating against Rust rules
**Example:** User edits `&mut self.123invalid` → parser accepts it
**Fix:** Validate extracted identifiers, sanitize if needed

### Issue #8: Keyword Collision (MEDIUM)
**Problem:** Field collector doesn't escape Rust keywords
**Example:** State binding named `"type"` → struct field `type: String` ← invalid
**Fix:** Add suffix to keywords (`type` → `type_value`)

---

## Readability / Maintainability

### Issue #5: Export Test Error Messages (MEDIUM)
**Problem:** When file write fails, test panics without explaining which file
**Fix:** Check file existence before read, provide clear error messages

### Issue #9: Deep Nesting Indentation (MEDIUM)
**Problem:** Fixed indentation at all levels → 40+ spaces at depth 10
**Fix:** Track nesting depth, use dynamic indentation

---

## Risk Assessment

### Current State (Without Fixes)
- ✗ **Data Loss Risk:** High — Parser failures can silently lose user edits
- ✗ **Compilation Risk:** High — Export can produce invalid Rust
- ✗ **Canvas Desync Risk:** High — Lazare round-trip can diverge
- ✗ **Test Clarity:** Low — Failures don't indicate root cause

### After Fixes
- ✓ **Data Loss Risk:** Low — All failures reported
- ✓ **Compilation Risk:** Low — All generated code validated
- ✓ **Canvas Desync Risk:** Low — Parser validates all extractions
- ✓ **Test Clarity:** High — Clear error messages

---

## Implementation Priority

### Phase 1 (CRITICAL — Do First)
1. **Fix unwraps in parser tests** (30 min)
   - Replace `.unwrap()` with `.expect("msg")`
   - Affects: lines 667-668, 782-784, 815
   
2. **Add malformed binding detection** (1 hour)
   - Detect `&mut self.` with no identifier
   - Report error instead of silent drop
   - Affects: `extract_binding_name()`

3. **Add identifier validation** (1 hour)
   - Validate all extracted identifiers
   - Sanitize if leading digit or keyword
   - Affects: `extract_binding_name()`, `parse_widget_line()`

### Phase 2 (HIGH — Do Next)
4. **Fix keyword collision** (30 min)
   - Escape keywords in `field_collector.rs`
   
5. **Fix string escaping** (45 min)
   - Handle Unicode surrogates in `rust.rs`
   
6. **Fix channel name sanitization** (45 min)
   - Validate identifiers in `rust_wiring.rs`

### Phase 3 (MEDIUM — Polish)
7. **Improve export test messages** (30 min)
   - Check file existence before read
   
8. **Fix indentation tracking** (1.5 hours)
   - Pass depth through emit functions
   - Use dynamic indentation

---

## Testing Strategy

### Unit Tests (add to parser.rs, rust.rs, field_collector.rs)
```rust
#[test]
fn test_malformed_binding_detected() { }

#[test]
fn test_identifier_validation() { }

#[test]
fn test_keyword_collision_escaped() { }

#[test]
fn test_string_literal_surrogates() { }

#[test]
fn test_channel_name_starts_with_letter() { }
```

### Integration Tests (run export → compile)
```bash
cargo test export_compiles --lib -- --ignored --nocapture
# Verifies: exported project runs `cargo check` successfully
```

### Regression Tests (Lazare round-trip)
```bash
cargo test parse_roundtrip --lib
# Verifies: emit → parse → reapply is lossless
```

---

## Files to Modify

| File | Changes | Lines |
|------|---------|-------|
| `src/codegen/parser.rs` | Tests: remove unwraps; Parser: validate bindings, detect malformed | 667-668, 782-784, 815, 479-492, ~300-330 |
| `src/codegen/rust.rs` | String escaping; keyword list | 8-10, 1-6 |
| `src/codegen/rust_wiring.rs` | Identifier sanitization | 20-35, ~200+ |
| `src/codegen/field_collector.rs` | Keyword escaping | 49-74 |
| `src/codegen/export.rs` | Test error messages | ~510-525 |
| `src/codegen/egui_emitter.rs` | Indentation tracking | 100-114, ~350-400 |

---

## Verification Checklist (Before Close)

- [ ] All unwraps replaced with `.expect("msg")` in tests
- [ ] Malformed bindings generate error diagnostics (not silent drop)
- [ ] Parser validates/sanitizes extracted identifiers
- [ ] Field collector escapes Rust keywords
- [ ] Channel/trait names produce valid identifiers
- [ ] String literals handle surrogates correctly
- [ ] Export tests clearly indicate missing files
- [ ] Deep nesting (10+ levels) produces readable code
- [ ] All new tests pass: `cargo test`
- [ ] No new clippy warnings: `cargo clippy -- -D warnings`
- [ ] Export generates compilable code: manual spot-check

---

## Deliverables in This Audit

1. **CODEGEN_AUDIT.md** — Detailed findings, failure scenarios, suggested test cases (THIS FILE)
2. **CODEGEN_AUDIT_FIXES.md** — Concrete code fixes with before/after examples
3. **CODEGEN_AUDIT_TESTS.md** — Test cases to verify each issue

---

## Recommendations

### Immediate Actions
1. Schedule Phase 1 fixes (2-3 hours total)
2. Run test suite after each fix
3. Do manual spot-check of export for a complex tree

### Long-term (Next Stage)
1. Create `src/codegen/identifiers.rs` — Dedicated identifier validation
2. Add property-based testing (`proptest`) for codegen
3. Create roundtrip testing harness (emit → parse → apply → compare)
4. Add integration test that compiles exported projects

### Documentation
1. Update `docs/ENGINEERING_INVARIANTS.md` with identifier rules
2. Add codegen best practices to `CONTRIBUTING.md`
3. Document Lazare sync contract in `docs/ARCHITECTURE.md`

---

## Questions for Code Review

1. **Test Code vs. Production:** Are the `.unwrap()`s in tests acceptable, or should all be `.expect()`?
2. **Error Recovery:** When parser fails, should we sanitize and continue, or stop and report error?
3. **Backward Compatibility:** Any concern about renaming `type` → `type_value` in existing projects?
4. **Export Compilation:** Should export auto-run `cargo check` or just generate files?

---

## Conclusion

The codegen pipeline has **9 correctness and soundness issues**, with **3 HIGH-severity bugs** that can cause:
- Silent data loss (Lazare round-trip divergence)
- Canvas/code desynchronization
- Export compilation failures

All issues have concrete fixes. **No structural redesign needed** — only validation gates and error handling.

**Estimated fix time:** 6-8 hours (Phase 1-3) + 1-2 hours testing.

---

**Report generated by:** RohKai Codegen Audit v1.0  
**Audit date:** 2026-01-XX  
**Status:** Ready for implementation
