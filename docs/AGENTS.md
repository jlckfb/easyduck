# Documentation rules

Applies to this documentation tree; repository-wide rules remain in [AGENTS.md](../AGENTS.md).

## Structure and audience

- Use [the documentation index](README.md). `en/` and `zh-CN/` contain matching ASCII filenames. The locale index is `README.md` in each directory.
- Keep user instructions, hardware requirements, implementation references, and verification limits in the relevant canonical guide. Link to the owner instead of creating competing explanations.
- Use root READMEs for project introduction and architecture. Keep code examples in `examples/`; maintenance programs belong in `tools/`, runtime programs in their existing code directories. Component READMEs additionally explain their own responsibilities, source files, callers, and boundaries directly; do not replace them with navigation-only stubs.
- Governance instructions, original third-party notices, and original license texts are intentionally unpaired. Do not translate away attribution, hashes, or legal scope.

## Bilingual maintenance

- Change both language pages in one documentation change. Both must cover the same sections and retain corresponding code examples, configuration keys, numeric limits, and warnings.
- Add a language switch below the title, pointing to the same filename in the other locale. Link related guides within the reader's locale; source-file links may leave the locale tree.
- Do not translate program names, service names, API methods, paths, parameter keys, hashes, or actual UI button labels. Add English explanations after Chinese UI labels where useful.
- Use UTF-8 and LF. Keep fenced commands copyable and their working directory explicit. Prefer repository-root commands, with explicit exceptions for existing component commands.
- Distinguish source behavior, offline checks, deployed software, and physical validation. Keep missing dependencies and unsupported features explicit.

## Terminology

| English | 简体中文 | Meaning |
| --- | --- | --- |
| daemon | 主控程序 | `robotd` during normal operation |
| policy | 动作策略／模型 | ONNX inference model, depending on context |
| control ownership | 控制权 | Which browser/gamepad input is accepted |
| torque release | 断力／松力矩 | Requested or confirmed servo torque-off; state which |
| hardware center | 硬件中位 | Permanent servo hardware setting |
| HOME | HOME／软件参考位置 | Software calibration reference; distinct from hardware center |
| installation rotation | 安装方向 | Sensor-to-torso rotation |
| receiver | 接收器 | USB receiver paired with a 2.4 GHz gamepad |
| physical validation | 实物验证 | Observed hardware behavior, not a build/test result |

## Verification

Run `python3 tools/check_docs.py` from the repository root. It checks pairs, language switches, section structure, code examples, local file/heading links, navigation, and shared media files. It does not check external websites, judge translation meaning, or validate physical behavior; review these separately.

When a document moves, update README navigation, notices, source comments, and example references that name it. Keep evidence and one-off migration scripts outside the repository. Do not add a documentation website, frontend framework, translation service, or package dependency without a concrete need.
