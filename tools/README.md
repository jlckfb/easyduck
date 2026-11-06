# Repository maintenance checks

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

Developer-side checks for repository documentation. This directory is separate from deployment scripts and robot programs; the current checker reads local files and reports documentation-integrity problems without contacting the robot.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [check_docs.py](check_docs.py) | UTF-8/LF, locale pairing, language switches, headings/examples, links, and navigation checks |
| [../docs/AGENTS.md](../docs/AGENTS.md) | Documentation terminology and update rules |
| [../AGENTS.md](../AGENTS.md) | Repository scope and contribution boundaries |

## Call and data flow

1. A maintainer runs python3 tools/check_docs.py from the repository root, or passes --root for an isolated source snapshot.
2. The checker walks relevant Markdown, parses live local references, compares language structure/examples, and returns failure when deterministic checks fail.

## Boundaries and change notes

- It does not verify external website availability, translation meaning, physical motion, or deployed services.
- Keep checks dependency-light and read-only. One-off migration scripts and evidence stay outside the repository; this script does not stage or commit.

## Further reading

- [Documentation index](../docs/en/README.md)
- [Repository maintenance rules](../AGENTS.md)
