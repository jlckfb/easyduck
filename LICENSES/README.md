# Original license texts

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

Stores original third-party license texts used by recorded components. These files are reference/legal records, not executable modules or a single license assignment for the whole repository. English and Chinese directory introductions point to the same authoritative originals.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [GPL-2.0-only.txt](GPL-2.0-only.txt) | Original GNU GPL version 2 text |
| [GPL-3.0-or-later.txt](GPL-3.0-or-later.txt) | GNU GPL version 3 text used with recorded or-later declarations |
| [../LICENSE](../LICENSE) | Root project license text |
| [../NOTICE](../NOTICE) | Attribution and source records |
| [../THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md) | Component-specific provenance and license scope |

## Call and data flow

1. Repository notices associate code/assets with their recorded terms; consult those records together with the original texts.
2. deploy/install.py copies license records into the application license directory for distribution with installed files.

## Boundaries and change notes

- Preserve original texts and attribution; do not translate them into replacement legal terms or infer that one filename licenses every component.
- A notice/hash check does not grant unresolved redistribution rights. This introduction does not change recorded component licensing.

## Further reading

- [Documentation index](../docs/en/README.md)
- [Repository maintenance rules](../AGENTS.md)
