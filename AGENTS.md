# EasyDuck repository maintenance

## Scope and evidence

- Maintain the application repository in this worktree. Board SDK, operating-system images, hardware designs, and model training are separate deliverables.
- Treat current implementation, configuration, locked dependencies, and model manifests as evidence. Documentation and past reports do not prove deployed state or physical acceptance.
- Preserve unrelated worktree and staged changes. Do not commit, push, deploy, restart board services, energize servos, or run physical actions without task authorization.
- Inspect first and explain the change before editing. Diagnose failures before changing behavior. Never put credentials, real private calibration, build outputs, or firmware images in public examples.

## Repository ownership

| Path | Owns |
| --- | --- |
| `README.md`, `README.zh-CN.md` | English/Chinese project introduction, system architecture, operator entry points, media showcase |
| `docs/en/`, `docs/zh-CN/` | Mirrored user and developer guides; see [documentation rules](docs/AGENTS.md) |
| `runtime/` | Rust programs and shared crates |
| `web/`, `calibration/` | Browser controller and exclusive hardware calibration service |
| `config/` | Uncalibrated profiles and hardware reference parameters |
| `models/` | Bundled policies, hashes, interfaces, and provenance |
| `deploy/` | Build/install/rollback tools and service/kernel inputs |
| `examples/` | Incomplete development examples, excluded from normal runtime |
| `assets/images/`, `assets/demos/` | README hardware photos and demonstration GIFs |
| `tools/` | Repository maintenance checks, not robot control programs |

## Documentation and localization

- Keep root READMEs paired. Keep every guide under both locale directories with the same ASCII filename and relative structure. Translate behavior, prerequisites, warnings, limitations, and examples together.
- Component/directory READMEs must contain responsibilities, real file structure, callers/dependencies/data flow, and change boundaries in both languages. A link-only entry is insufficient. Link to canonical guides for detailed operating procedures instead of duplicating them.
- Preserve the user-requested architecture analysis and operator commands in root READMEs. Generic skill defaults must not remove them.
- Retain original legal notices and license texts as authoritative records; do not invent translated licensing terms. Link to them from both locales.
- Keep the runtime UI's current Chinese labels visible when explaining buttons in English. Documentation localization does not imply that the UI itself is localized.
- Add photos to `assets/images/` and GIFs to `assets/demos/`, using shared files and locale-specific captions. Embed only existing media; omit unused placeholder sections when requested. Never add broken image URLs or imply an unperformed motion test.
- Update all relative links when moving documents or examples. Keep command working directories explicit. Do not reorganize live runtime files as part of a documentation task.

## Checks and contributions

From the repository root:

```sh
python3 tools/check_docs.py
git diff --check
git diff --cached --check
```

For implementation changes, follow the relevant [deployment/test instructions](docs/en/deployment.md), [policy contract](docs/en/models.md), and [IMU contract](docs/en/imu-drivers.md). Native tests and ARM64 builds do not establish physical behavior. Existing local-only tests excluded by Git are supplementary, not reproducible checkout prerequisites.

Stage explicit paths for the requested functional slice; never use a blanket add in a mixed tree. Review the staged file list and validate the actual slice before committing. Commit messages use `type(scope): #YYMMDD description`, following the user's current format requirements. Creation of documentation does not authorize a commit or push.
