---
name: harbor
description: Create, maintain, discover, start, stop, and inspect Harbor Task and Group workflows through Harbor YAML files, workspace search paths, logs, panels, and the harbor_core HTTP API. Use when a request mentions Harbor tasks, groups, task orchestration, harbor_taskcfg, workspace search_paths, task logs, Web Panels, ROS2 programs launched by Harbor, or operating a running Harbor instance.
---

# Harbor

Use Harbor to turn repeatable commands into project-owned Task and Group workflows, then operate them through `harbor_core`.

## Workflow

1. Identify whether the request is configuration work, runtime operation, or both.
2. Read only the relevant reference files listed below.
3. For configuration work, locate the project and current workspace before editing YAML.
4. Stop a running Task before changing its definition.
5. Preserve unspecified commands, fields, environment variables, and configuration.
6. Validate YAML paths, Task references, UUID uniqueness, and the Harbor version rule.
7. For runtime work, call the HTTP API directly instead of instructing the user to click the GUI.
8. Report the files changed and runtime actions performed.

## Choose the Configuration Type

- Create a **Task** for one runnable command.
- Create a **Group** only when the user explicitly requests composition, ordering, or orchestration of multiple Tasks.
- Keep project configuration under `harbor_taskcfg/`; do not create a Group alongside every Task.
- Write `description` in Chinese when practical.

## Read References as Needed

- Task or Group creation workflow: [references/task-workflow.md](references/task-workflow.md)
- Project paths and discovery: [references/task-paths.md](references/task-paths.md)
- Task YAML schema: [references/task-yaml.md](references/task-yaml.md)
- Group YAML schema: [references/group-yaml.md](references/group-yaml.md)
- Workspace and `search_paths`: [references/settings.md](references/settings.md)
- Version rules: [references/version.md](references/version.md)
- Runtime HTTP API and logs: [references/web-api.md](references/web-api.md)
- Safety checklist: [references/safety.md](references/safety.md)

## Operational Rules

- Treat `(prefix_path, id)` as the configuration lookup key and `uuid` as the global runtime identity.
- Allow the core to generate a missing UUID; never copy a UUID from another Task or Group.
- Keep one running instance per Task UUID across local, remote, and workspace views.
- Read `harbor --version` when creating YAML or repairing an old version; never invent or increment a version.
- Remember that the default core API has no authentication. Do not expose it beyond trusted networks.
