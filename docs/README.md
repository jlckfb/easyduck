# Bilingual project documentation

**English** | [简体中文](README.zh-CN.md)

## Responsibilities

Organizes canonical user/developer guides in matching locale trees. Directory/module READMEs explain local responsibilities and call relationships; these guides own detailed procedures, interface explanations, and verification limits.

## File structure

| File / directory | Responsibility |
| --- | --- |
| [en/README.md](en/README.md) | English guide index |
| [zh-CN/README.md](zh-CN/README.md) | Simplified Chinese guide index |
| [AGENTS.md](AGENTS.md) | Terminology, bilingual update rules, and evidence boundaries |
| [en/imu-drivers.md](en/imu-drivers.md) | Driver extension and coordinate/time contract |
| [en/models.md](en/models.md) | Policy observation/action contract and provenance |

## Call and data flow

1. Root and component READMEs link readers to the guide in their chosen language; English/Chinese guides use the same filenames.
2. tools/check_docs.py checks local references and paired structure/examples. Maintainers still review implementation meaning and translated warnings.

## Boundaries and change notes

- Documentation does not override code/configuration or establish deployed state. Keep source behavior, offline checks, and physical validation separate.
- Do not place executable robot helpers here. Examples belong under examples, maintenance programs under tools, and original license records remain authoritative.

## Further reading

- [Documentation index](en/README.md)
- [Repository maintenance rules](../AGENTS.md)
