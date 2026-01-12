# talaria

Talaria is a spec-driven CLI and TUI for the public Hermes API. All request/response types are derived from the bundled `openapi.json` so the tools stay in lockstep with the published contract.

## Quick start

```bash
# configure (env vars override file values)
export HERMES_API_KEY=sk_...
export HERMES_BASE_URL=https://api.hermes-api.dev
# optional: Supabase image upload config
export SUPABASE_URL=https://xxxx.supabase.co
export SUPABASE_SERVICE_ROLE_KEY=sb_sr_...
export SUPABASE_BUCKET=images-bucket
export SUPABASE_UPLOAD_PREFIX=talaria/$(date +%Y-%m-%d)/

# health check
cargo run -p talaria-cli -- health

# hsuf enrich
cargo run -p talaria-cli -- hsuf-enrich --images https://example.com/img.jpg
# or capture+upload in one go (camera feature build): 
cargo run -p talaria-cli -- hsuf-enrich --capture 3 --include-usage

# create listing
cargo run -p talaria-cli -- listings create \
  --images https://example.com/img1.jpg https://example.com/img2.jpg \
  --merchant-location-key loc-1 \
  --fulfillment-policy-id pol-fulfill \
  --payment-policy-id pol-pay \
  --return-policy-id pol-return

# continue listing with overrides
cargo run -p talaria-cli -- listings continue \
  --sku sku-123 \
  --merchant-location-key loc-1 \
  --fulfillment-policy-id pol-fulfill \
  --payment-policy-id pol-pay \
  --return-policy-id pol-return \
  --override-category '{"id":"cat","tree_id":"tree","label":"Label","confidence":0.9,"rationale":"User override"}'

# upload a directory then create a listing
cargo run -p talaria-cli -- listings create \
  --images-from-dir ./photos \
  --merchant-location-key loc-1 \
  --fulfillment-policy-id pol-f \
  --payment-policy-id pol-p \
  --return-policy-id pol-r

# pricing quote
cargo run -p talaria-cli -- pricing quote --images https://example.com/img.jpg \
  --merchant-location-key loc-1 --fulfillment-policy-id pol-f --payment-policy-id pol-p --return-policy-id pol-r

# usage table output
cargo run -p talaria-cli -- usage list --format table

# capture/upload helpers
cargo run -p talaria-cli -- images capture --count 2 --upload
cargo run -p talaria-cli -- images upload --paths a.jpg b.jpg

# TUI (async, ratatui-based)
cargo run -p talaria-tui
```

## Nix dev shell

If you want a reproducible dev environment for the camera TUI:

```bash
nix develop
cargo run -p talaria-tui
```

The pinned inputs live in `flake.lock`; `nix develop` will use those by default.

## Camera TUI

```bash
# optional: force camera resolution (e.g., 3840x2160)
export TALARIA_CAMERA_RESOLUTION=3840x2160

# camera control + preview (terminal in Kitty/WezTerm, window fallback elsewhere)
cargo run -p talaria-tui
```

Keybindings:

- `q` quit
- `t` camera on/off (stream + preview)
- `v` device picker
- `d` / `D` device index down/up
- `c` capture one frame
- `b` capture burst (defaults to 10)
- `h` toggle help

Config file (optional) lives at `~/.config/talaria/config.toml`:

```toml
base_url = "https://api.hermes-api.dev"
api_key = "sk_..."
supabase_url = "https://xxxx.supabase.co"
supabase_service_role_key = "sb_sr_..."
supabase_bucket = "images-bucket"
supabase_upload_prefix = "talaria/"
```

Never print secrets; the CLI redacts API keys in `talaria config doctor`.
