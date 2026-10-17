# Local shortcuts. CI lives in .github/workflows; `just build` reproduces one runner's job natively.

check:
    ci/check-version.sh
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

# Live network tests (GitHub releases + Adoptium API, local Java scan).
net-test:
    VOID_NET_TESTS=1 cargo test -p launcher-core --test live_api -- --ignored --nocapture

run:
    cargo run -p void-launcher

# Build and package this machine's target: linux-x86_64 | windows-x86_64 | macos-universal
build target:
    ci/build.sh {{target}}

# Assemble dist/ (manifest.json, SHA256SUMS) from whatever is in out/.
dist:
    ci/make-dist.sh

# Bump Cargo.toml, commit and push a release tag; CI does the rest.
release version:
    sed -i 's/^version = ".*"/version = "{{version}}"/' Cargo.toml
    cargo update -w --offline
    git add Cargo.toml Cargo.lock
    git commit -m "Release {{version}}"
    git tag "v{{version}}"
    git push origin HEAD "v{{version}}"
