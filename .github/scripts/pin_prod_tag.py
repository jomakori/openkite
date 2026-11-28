#!/usr/bin/env python3
"""Move one app's production image tag in the GitOps chart values.

The production tag is a single line in a file whose comments and blank lines are
part of how it reads. Rewriting the document through a YAML library reflows all of
it, so this walks to `<app>.environments.production.tag` and replaces that line
only.

Usage: pin_prod_tag.py <values.yaml> <app> <tag>
"""

import pathlib
import sys


def block_end(lines, start):
    """Index of the first line that leaves the block opened at `start`."""
    indent = len(lines[start]) - len(lines[start].lstrip())
    for i in range(start + 1, len(lines)):
        stripped = lines[i].strip()
        if stripped and not lines[i].startswith(" " * (indent + 1)):
            return i
    return len(lines)


def main():
    values_path, app, tag = pathlib.Path(sys.argv[1]), sys.argv[2], sys.argv[3]
    lines = values_path.read_text().splitlines(keepends=True)

    app_line = next(
        (i for i, line in enumerate(lines) if line.rstrip("\n") == f"{app}:"), None
    )
    if app_line is None:
        sys.exit(f"'{app}:' is not a top-level key in {values_path}")

    prod_line = next(
        (
            i
            for i in range(app_line, block_end(lines, app_line))
            if lines[i].strip() == "production:"
        ),
        None,
    )
    if prod_line is None:
        sys.exit(f"'{app}' has no environments.production block")

    tag_line = next(
        (
            i
            for i in range(prod_line, block_end(lines, prod_line))
            if lines[i].strip().startswith("tag:")
        ),
        None,
    )
    if tag_line is None:
        sys.exit(f"'{app}.environments.production' has no tag")

    indent = lines[tag_line][: len(lines[tag_line]) - len(lines[tag_line].lstrip())]
    lines[tag_line] = f"{indent}tag: {tag}\n"
    values_path.write_text("".join(lines))


if __name__ == "__main__":
    main()
