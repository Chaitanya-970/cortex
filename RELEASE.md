# Cortex Release Process

Cortex should use explicit, reproducible release processes.

## Target platforms

- Linux x86_64
- Linux ARM64
- macOS ARM64
- macOS x86_64
- Windows x86_64

## Release flow

```text
version/tag
    ↓
full CI
    ↓
cross-platform builds
    ↓
artifact tests
    ↓
checksums
    ↓
artifact verification
    ↓
GitHub Release
    ↓
optional package publication
```

## Checklist

- [ ] Version updated
- [ ] Changelog updated
- [ ] Documentation updated
- [ ] Full CI passes
- [ ] Evaluation suite passes
- [ ] Release artifacts built
- [ ] Checksums generated
- [ ] Artifacts verified
- [ ] Release notes prepared
- [ ] Git tag created
- [ ] GitHub Release published

## Future distribution

Potential distribution channels:

- crates.io
- GitHub Releases
- OCI images
- Homebrew
