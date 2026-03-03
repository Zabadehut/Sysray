# Contributing

Thanks for considering a contribution to Pulsar.

Pulsar is still in an early stage. The main goal right now is to build a technically solid foundation before making broad product claims. Contributions that improve correctness, portability, performance, tests, and clarity are the most useful.

## Principles

- Keep claims honest and aligned with the current state of the code
- Prefer small, reviewable changes over broad rewrites
- Preserve cross-platform architecture boundaries
- Do not scatter OS-specific logic through collectors when it belongs in `src/platform/`
- Favor correctness and measurement over assumptions about performance

## Development Setup

Requirements:

- Rust stable
- `cargo fmt`
- `cargo clippy`

Typical workflow:

```bash
cargo fmt
cargo build
cargo clippy --all-targets --all-features -- -D warnings
```

## Project Structure

```text
src/
├── collectors/   # metric collection
├── platform/     # OS-specific implementations
├── pipeline/     # derived metrics
├── engine/       # scheduler and registry
├── exporters/    # output formats
├── tui/          # terminal UI
└── api/          # HTTP server
```

## Contribution Priorities

Current high-value areas:

- Linux correctness and regression coverage
- Real macOS implementations
- Real Windows implementations
- Tests and CI hardening
- Documentation and launch readiness
- Performance measurement and profiling

## Pull Requests

Please keep pull requests focused.

A good PR should:

- explain the problem clearly
- explain the chosen approach
- mention platform impact
- mention compatibility impact
- include tests when practical

If the PR changes behavior or schema, call that out explicitly.

## Coding Expectations

- Use idiomatic Rust
- Keep `cargo fmt` clean
- Keep `clippy` clean
- Avoid unnecessary dependencies
- Prefer backward-compatible data evolution
- Treat Linux/macOS/Windows support claims carefully

## Reporting Issues

Useful issue reports include:

- OS and version
- Pulsar version or commit
- exact command used
- expected behavior
- actual behavior
- sample output or logs if relevant

## Security

For security-related issues, see [SECURITY.md](SECURITY.md).
