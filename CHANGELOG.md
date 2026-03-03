# Changelog

All notable changes to this project will be documented in this file.

The format is inspired by Keep a Changelog, and this project aims to follow Semantic Versioning once public releases begin.

## [Unreleased]

### Added

- Cross-platform `platform/` abstraction layer with Linux implementation and macOS/Windows stubs
- Scheduler-integrated computed metrics pipeline
- Backward-compatible snapshot deserialization via `serde(default)`
- Service management scaffolding for Linux, macOS, and Windows
- Initial public-facing `README.md`

### Changed

- Collectors now route OS-specific logic through `src/platform/`
- Configuration extended with pipeline settings and thresholds
- Snapshot schema extended with computed metrics and alerts

### Notes

- Linux is currently the only platform with real collector implementations
- macOS and Windows remain architecture-ready but incomplete
