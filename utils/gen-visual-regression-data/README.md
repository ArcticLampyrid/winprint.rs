# `gen_visual_regression_data`

Regenerates every fixture under `test_data/visual_regression/` from scratch.

> [!NOTE]
> **You usually don't need to run this.** The repo ships the fixtures
> pre-generated and version-controlled. `cargo test` uses them as-is, and any
> given run of this script produces byte-identical files, so re-running it
> just yields the same data that's already there.

## When to run

Re-generate the fixtures only when one of these applies:

- The drawing code in `run.py` changed (different colour swatches, new
  pages, a new probe for a known regression, …).
- A new printer flavour is being added under `test_data/visual_regression/`
  and needs matching fixtures.
- A bump in one of the upstream renderers produces a meaningfully different
  baseline and the committed expected images need to be refreshed.

If you just want to run the tests, skip this script entirely.

## What it produces

```text
test_data/visual_regression/
├── data.pdf
├── data.tiff
└── data.xps
```

## Determinism

Byte-identical re-runs are non-negotiable — the fixtures are committed and
any non-determinism would churn the repo on every regeneration. The script
pins every known timestamp source:

- `SOURCE_DATE_EPOCH=1700000000` is set before any tool runs (reportlab and
  Ghostscript's `xpswrite` both honour it for file timestamps / ZIP mtimes).
- reportlab runs in `invariant=True` mode so it doesn't embed the current
  time, random IDs, or creator/producer fingerprints.
- All intermediate per-page TIFFs are written with `-tiffcompression none`
  and TIFFs are packed with `tiffcp -c zip`, avoiding the tagged mtimes and
  random-ordered dictionary entries that some encoders emit.

If two back-to-back runs on the same machine produce different bytes,
that's a bug worth chasing before committing fixtures.

## Dependencies

The script is pure Python 3, but it shells out to a handful of command-line
tools. On Arch Linux:

```bash
sudo pacman -S python python-reportlab poppler libtiff ghostscript
```

| Tool        | Package (Arch)     | Role                    |
| ----------- | ------------------ | ----------------------- |
| `python3`   | `python`           | Host language           |
| `reportlab` | `python-reportlab` | Draws `pdf/origin.pdf`  |
| `tiffcp`    | `libtiff`          | Packs TIFFs             |
| `gs`        | `ghostscript`      | Converts PDF → XPS/TIFF |

The script fails fast with a friendly message if any of the binaries is
missing from `PATH`.

## Usage

From anywhere:

```bash
python utils/gen_visual_regression_data/run.py
```

Output goes straight into `test_data/visual_regression/`, overwriting any
existing files.