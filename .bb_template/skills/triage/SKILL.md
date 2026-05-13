---
name: triage
description: Inbox triage and ticket routing skill for BB-PM agent.
---

# triage — Inbox Triage Skill

## Trigger Conditions

This skill is automatically loaded when:

- The agent is asked to process inbox notes
- The agent is asked to classify or route tickets
- The agent encounters new handoff notes that need triage

---

## Core Rules

### Classification

Each inbox note should be classified into one of:

| Category | Action |
|----------|--------|
| **Actionable** | Create or update a ticket with the note's content |
| **Informational** | Append to an existing ticket's progress section |
| **Stale** | Archive or delete if older than 7 days with no actionable content |

### Routing

- If the note mentions a specific ticket ID → append to that ticket
- If the note describes a new task → create a new ticket in the appropriate lane
- If the note is a status update → update the referenced ticket's status

### Output Format

Always output a JSON summary:

```json
{
  "processed": 3,
  "created_tickets": ["000051"],
  "updated_tickets": ["000048"],
  "archived_notes": ["old-note.md"],
  "skipped": 0
}
```
