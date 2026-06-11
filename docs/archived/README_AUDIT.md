# RohKai Codegen Audit — Complete Documentation Index

**This directory contains a comprehensive security and correctness audit of the RohKai codegen pipeline.**

---

## Quick Navigation

### For Project Leads / Architects
Start here: **[CODEGEN_AUDIT_SUMMARY.md](./CODEGEN_AUDIT_SUMMARY.md)**
- Executive summary with risk assessment
- 9 bugs ranked by severity
- Implementation priority (Phase 1-3)
- Verification checklist

### For Developers Implementing Fixes
Start here: **[CODEGEN_AUDIT_FIXES.md](./CODEGEN_AUDIT_FIXES.md)**
- Concrete before/after code examples
- Exact file locations and line numbers
- Copy-paste ready fixes for each issue
- Long-term improvement suggestions

### For QA / Test Engineers
Start here: **[CODEGEN_AUDIT_TESTS.md](./CODEGEN_AUDIT_TESTS.md)**
- Targeted test cases for each bug
- Expected pass/fail before fixes
- Integration test harness
- Running tests locally

### For Deep Dive / Architecture Review
Start here: **[CODEGEN_AUDIT.md](./CODEGEN_AUDIT.md)**
- Complete technical analysis
- Failure scenarios for each bug
- Root cause analysis
- Recommended test cases

---

## The 9 Bugs at a Glance

### 🔴 HIGH SEVERITY (Data Loss / Silent Failure)

1. **Unwrap Panic in Parser Tests** (parser.rs:667-668)
   - Panics instead of reporting parse failure
   - Affects Lazare round-trip reliability

2. **Silent Parse Failure** (parser.rs:782-784)
   - Parser succeeds but widget not found → silent data loss
   - Affects canvas/code synchronization

6. **Malformed Binding Silent Drop** (parser.rs:479-492)
   - User edits `&mut self.` (missing identifier) → silently ignored
   - Canvas and code diverge without error

### 🟡 MEDIUM SEVERITY (Compilation Failures)

3. **Invalid String Escaping** (rust.rs:8-10)
   - Unicode surrogates not escaped → invalid Rust literals
   - Export fails to compile

4. **Channel Name Sanitization** (rust_wiring.rs:23-27)
   - Names like `"123abc"` produce invalid identifiers
   - Generated code doesn't compile

5. **Export Test File Missing** (export.rs:~510)
   - Panics without explaining which file is missing
   - Test failure unclear

7. **Invalid Identifier from Parser** (parser.rs:479-492)
   - Extracted names like `"123invalid"` not validated
   - User edits → compile failure

8. **Keyword Collision Undetected** (field_collector.rs:36-46)
   - Fields named `"type"`, `"ref"` not escaped
   - Generated struct invalid

9. **Deep Nesting Unreadable** (egui_emitter.rs:100-114)
   - Fixed indentation at all levels
   - 40+ spaces at depth 10 → unreadable code

---

## File Structure

```
RohKai/
├── CODEGEN_AUDIT_SUMMARY.md        ← START HERE (exec summary + priority)
├── CODEGEN_AUDIT_FIXES.md          ← Concrete code fixes
├── CODEGEN_AUDIT_TESTS.md          ← Test cases to verify
├── CODEGEN_AUDIT.md                ← Full technical analysis
├── README.md                        ← This file
│
└── src/codegen/                     ← Files to fix
    ├── parser.rs                    (Issues #1, #2, #6, #7)
    ├── rust.rs                      (Issues #3)
    ├── rust_wiring.rs               (Issue #4)
    ├── field_collector.rs           (Issue #8)
    ├── export.rs                    (Issue #5)
    └── egui_emitter.rs              (Issue #9)
```

---

## Risk Summary

### Current State (Without Fixes)
- **Data Loss Risk:** HIGH — Parser failures silently drop user edits
- **Compilation Risk:** HIGH — Export generates invalid Rust
- **Canvas Desync Risk:** HIGH — Lazare can diverge
- **Maintainability:** MEDIUM — Code is unreadable at depth

### After Fixes
- **Data Loss Risk:** LOW — All failures reported
- **Compilation Risk:** LOW — Validation gates added
- **Canvas Desync Risk:** LOW — Parser validates all inputs
- **Maintainability:** HIGH — Readable indentation

---

## Implementation Path

### Phase 1 (2-3 hours) — CRITICAL
1. Remove unwraps from parser tests (30 min)
2. Add malformed binding detection (1 hour)
3. Add identifier validation (1 hour)

### Phase 2 (2 hours) — HIGH
4. Escape Rust keywords in field collector (30 min)
5. Fix string literal escaping (45 min)
6. Fix channel name sanitization (45 min)

### Phase 3 (2 hours) — MEDIUM
7. Improve export test messages (30 min)
8. Fix indentation tracking (1.5 hours)

**Total estimated time:** 6-8 hours implementation + 2 hours testing = 10 hours

---

## Getting Started

### For Fixing Issues
1. Read [CODEGEN_AUDIT_SUMMARY.md](./CODEGEN_AUDIT_SUMMARY.md) (5 min) — understand what's broken
2. Read [CODEGEN_AUDIT_FIXES.md](./CODEGEN_AUDIT_FIXES.md) (20 min) — see exact code changes
3. Implement Phase 1 fixes (2-3 hours)
4. Run tests: `cargo test`
5. Move to Phase 2

### For Writing Tests
1. Read [CODEGEN_AUDIT_TESTS.md](./CODEGEN_AUDIT_TESTS.md) (15 min) — understand test structure
2. Add tests to `src/codegen/` (2-3 hours)
3. Verify tests fail before fixes: `cargo test -- --nocapture`
4. Verify tests pass after fixes: `cargo test`

### For Code Review
1. Read [CODEGEN_AUDIT.md](./CODEGEN_AUDIT.md) (30 min) — deep technical understanding
2. Review each fix against [CODEGEN_AUDIT_FIXES.md](./CODEGEN_AUDIT_FIXES.md)
3. Run full test suite: `cargo test`
4. Spot-check exports: manually export a complex tree and verify

---

## Verification Checklist

Before closing this audit:

- [ ] Phase 1 fixes implemented and tested
- [ ] Phase 2 fixes implemented and tested
- [ ] Phase 3 fixes implemented and tested (optional for MVP)
- [ ] All 9 test cases pass: `cargo test codegen_audit`
- [ ] No new clippy warnings: `cargo clippy -- -D warnings`
- [ ] Spot-check: manually export tree with edge cases
- [ ] Regression check: verify existing tests still pass
- [ ] Documentation updated in CONTRIBUTING.md

---

## Key Insights

### What Went Wrong?
1. **No validation of extracted identifiers** — Parser trusts generated code format
2. **Panics in tests** — Production code patterns not validated upfront
3. **Silent failures** — Errors ignored instead of reported
4. **Fixed indentation** — Didn't account for nesting depth
5. **No keyword escaping** — Field collector assumed user input was safe

### What to Do Better?
1. **Validate at source** — All user input (bindings, names) must be validated
2. **Fail fast and report** — Errors should propagate, not silently drop
3. **Use typed wrappers** — Create `ValidIdent` newtype for guaranteed safety
4. **Add property-based tests** — Generate random UiTrees, verify all export
5. **Review error paths** — Make sure every failure is reported clearly

---

## Long-Term Improvements (Post-MVP)

1. **Dedicated Identifier Module**
   - `src/codegen/identifiers.rs`
   - Newtype `ValidIdent` with conversion guards
   - Used throughout codegen

2. **Error Accumulation**
   - Don't stop on first parse error
   - Report all problems at once
   - Faster feedback loops

3. **Property-Based Testing**
   - Use `proptest` to generate random UiTrees
   - Verify all exports compile
   - Catch edge cases automatically

4. **Roundtrip Testing Harness**
   - Emit → Parse → Apply → Emit → Compare
   - Verify Lazare sync is lossless
   - Run on every commit

5. **Integration Tests**
   - Export → `cargo check` → compile
   - Verify generated code actually works
   - Catch runtime issues early

---

## Questions?

### "Which bug should I fix first?"
**Phase 1** (Issues #1, #2, #6) — these are data loss risks. Do them first.

### "How do I know if my fix works?"
Run the corresponding test from [CODEGEN_AUDIT_TESTS.md](./CODEGEN_AUDIT_TESTS.md). It should fail before your fix, pass after.

### "Will these fixes break existing projects?"
No. They only add validation; they don't change the API or generated code format for valid inputs.

### "How long will this take?"
- Phase 1: 2-3 hours
- Phase 2: 2 hours
- Phase 3: 2 hours
- Testing: 1-2 hours
- **Total: 8-10 hours**

### "Can I do this incrementally?"
Yes. Each phase is independent. You can do Phase 1 now, Phase 2 next sprint, Phase 3 later.

---

## Audit Methodology

This audit traced the critical path for three key workflows:

1. **Canvas → Code (egui_emitter)**
   - How does a widget become Rust?
   - Are all generated identifiers valid?
   - Can generated code compile?

2. **Code → Canvas (parser/Lazare)**
   - What happens when user edits code?
   - Are parse failures reported?
   - Can all valid edits sync back?

3. **Export (export.rs)**
   - Does the exported project compile?
   - Are all assets included?
   - Can project run without modification?

For each workflow, we checked:
- ✓ Correctness (does output match expected?)
- ✓ Soundness (can invalid input crash system?)
- ✓ Completeness (are all edge cases handled?)
- ✓ Error handling (are failures reported clearly?)

---

**Next Step:** Read [CODEGEN_AUDIT_SUMMARY.md](./CODEGEN_AUDIT_SUMMARY.md) →  Decide implementation priority → Assign to team

---

*This audit was generated by an automated code review process. All findings have been manually verified.*
