---
name: jura-cli
description: Fetch Jira ticket information using the jura CLI. Use when you need context about the Jira task currently being worked on, or to list all tickets assigned to the user.
---

# Jura Skill

This skill lets you retrieve Jira ticket data via the `jura` CLI, which reads from a local cache populated by the `jura` TUI. No network calls are made — only locally cached data is accessible.

## Security & Privacy
`jura` reads from a local cache populated when the user opens the Mine tab in the jura TUI. The user controls what data is available.

## Commands

### Get the ticket for the current branch
```
jura current
```
Extracts the Jira ticket key from the current git branch name and returns full details. Use this first when you need context about what is being worked on.

### List all assigned tickets
```
jura tickets
```
Returns a JSON array of all tickets in the local cache. The ticket matching the current branch has `"checked_out": true`.
```json
[{ "key": "PROJ-123", "summary": "...", "status": "In Progress",
   "type": "Story", "priority": "Medium", "assignee": "Jane Doe",
   "checked_out": true }]
```

### Get full details for a specific ticket
```
jura ticket PROJ-123
```
Returns a JSON object with all fields:
```json
{ "key": "PROJ-123", "summary": "...", "description": "...",
  "status": "In Progress", "type": "Story", "priority": "Medium",
  "assignee": "Jane Doe", "components": ["Frontend"],
  "labels": ["auth"], "parent": { "key": "PROJ-100", "summary": "..." },
  "sprint": "Sprint 42" }
```

## Workflow

### Understand the current task
Run `jura current` to get full details for the ticket linked to the current git branch.

### Browse all assigned tickets
Run `jura tickets` to get a list, then `jura ticket <KEY>` for any ticket needing more detail.

## Handling Missing Data
If a command returns `{"error": "No tickets cached..."}` or says a ticket was not found, ask the user to open the jura TUI — loading the Mine tab automatically populates the cache.

## Important Notes
- Read-only — cannot comment, transition, or create tickets
- Keys are case-insensitive (`proj-123` and `PROJ-123` both work)
