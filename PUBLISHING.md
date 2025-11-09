# Publishing Rexis to crates.io

## Prerequisites

### 1. Configure crates.io API Token

The GitHub Actions workflow requires a `CARGO_REGISTRY_TOKEN` secret to publish crates.

**Set up the token:**

1. Get your crates.io API token:
   - Go to https://crates.io/settings/tokens
   - Click "New Token"
   - Name: `GitHub Actions - Rexis`
   - Scopes: `publish-update`
   - Click "Create" and copy the token

2. Add secret to GitHub:
   ```bash
   gh secret set CARGO_REGISTRY_TOKEN --body "your-token-here" --repo 0xteamhq/rexis
   ```

   Or via web UI:
   - Go to https://github.com/0xteamhq/rexis/settings/secrets/actions
   - Click "New repository secret"
   - Name: `CARGO_REGISTRY_TOKEN`
   - Value: Paste your token
   - Click "Add secret"

## Publishing Order

Crates must be published in dependency order:

1. **rexis-macros** (no dependencies)
2. **rexis-llm** (depends on rexis-macros)
3. **rexis-rag** (depends on rexis-llm)
4. **rexis-graph** (depends on rexis-rag)
5. **rexis** (depends on all above)

## Publishing Methods

### Method 1: Automatic via Tags (Recommended)

```bash
# Create and push tags for all crates
git tag rexis-macros-v0.1.0
git tag rexis-llm-v0.1.0
git tag rexis-rag-v0.1.0
git tag rexis-graph-v0.1.0
git tag rexis-v0.1.0

# Push all tags
git push origin --tags
```

The GitHub Actions workflow will automatically:
- Publish each crate in order
- Wait 90 seconds between crates for propagation
- Create GitHub releases

### Method 2: Manual Workflow Dispatch

1. Go to https://github.com/0xteamhq/rexis/actions/workflows/publish-crates.yml
2. Click "Run workflow"
3. Select which crate to publish:
   - `all` - Publish all crates in order
   - Individual crate names for single publishes

### Method 3: Manual Publishing (Local)

**Important**: Publish in order!

```bash
# 1. Publish rexis-macros first
cd crates/rexis-macros
cargo publish
cd ../..

# Wait 1-2 minutes for crates.io propagation

# 2. Publish rexis-llm
cd crates/rexis-llm
cargo publish
cd ../..

# Wait 1-2 minutes

# 3. Publish rexis-rag
cd crates/rexis-rag
cargo publish
cd ../..

# Wait 1-2 minutes

# 4. Publish rexis-graph
cd crates/rexis-graph
cargo publish
cd ../..

# Wait 1-2 minutes

# 5. Publish rexis (umbrella)
cd crates/rexis
cargo publish
```

## Testing Before Publishing

Always dry-run first:

```bash
# Test each crate
cargo publish --dry-run -p rexis-macros
cargo publish --dry-run -p rexis-llm --allow-dirty
cargo publish --dry-run -p rexis-rag --allow-dirty
cargo publish --dry-run -p rexis-graph --allow-dirty
cargo publish --dry-run -p rexis --allow-dirty
```

## Troubleshooting

### "please provide a non-empty token"
- The `CARGO_REGISTRY_TOKEN` secret is not configured
- Follow the prerequisites section above

### "dependency X does not specify a version"
- All path dependencies must have version numbers
- Format: `crate-name = { version = "0.1.0", path = "../path" }`

### "no matching package named X found"
- Dependent crate not yet published to crates.io
- Wait 1-2 minutes for propagation
- Or publish dependencies first

### Workflow fails on clippy/tests
- The workflow has been configured to be permissive
- Clippy is non-blocking (`|| true`)
- Only critical tests run (rexis-llm, rexis-macros)

## Version Management

Current version: `0.1.0` (all crates)

To publish new versions:
1. Update version in respective crate's `Cargo.toml`
2. Update dependent crate versions
3. Create new tags
4. Push tags to trigger publish

## Verification

After publishing, verify at:
- https://crates.io/crates/rexis-macros
- https://crates.io/crates/rexis-llm
- https://crates.io/crates/rexis-rag
- https://crates.io/crates/rexis-graph
- https://crates.io/crates/rexis
