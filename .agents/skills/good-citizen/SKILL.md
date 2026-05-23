---
name: good-citizen
description: Apply this discipline to every coding session. Before finishing 
any task, scan adjacent code for opportunities to improve clarity, consistency, 
or correctness — without changing behavior. Leave the codebase cleaner than 
you found it.
---

# The Good Citizen Principle

## What It Is

When you touch a file to implement a change, you are not done when the 
change works. You are done when the surrounding code is at least as clean 
as it was before you arrived — ideally cleaner.

This is analogous to "static discipline" in electrical engineering: a signal 
leaving a system must be at least as clean as the signal that entered it.

## What It Requires

**Before finishing any session:**
- Scan files you touched for adjacent issues: dead code, inconsistent naming, 
  stale comments, redundant logic, copy-paste patterns that should be extracted
- Fix what you can without changing behavior
- If a fix would change behavior or require significant refactoring, add a 
  comment: `// CITIZEN: [describe issue]` and move on

**What counts as a Good Citizen action:**
- Removing unused imports, variables, or dead code paths
- Renaming a variable that was misleadingly named
- Extracting a repeated pattern into a helper function
- Adding a clarifying comment to logic that is correct but non-obvious
- Fixing a clippy warning that wasn't causing your current task

**What does NOT count:**
- Refactoring working architecture because you'd have designed it differently
- Changing behavior to fix something that wasn't broken
- Large-scale restructuring during a focused fix session
- "Cleaning up" code you don't fully understand yet

## The Double-Edged Sword

A Good Citizen who misunderstands the system accelerates technical debt 
faster than a politician who does nothing. 

**Rule:** if you are not certain why code is written the way it is, add a 
`// CITIZEN: [question]` comment instead of changing it. Never "clean up" 
logic you cannot fully trace.

## In Practice

At the end of every session, before cargo check, ask:
- Did I leave any file worse than I found it?
- Are there any obvious adjacent issues I can fix in under 2 minutes?
- Did I introduce any new TODOs, dead code, or inconsistencies?

If yes to the first, fix it. If yes to the second, fix it. If yes to the 
third, fix it before closing.
