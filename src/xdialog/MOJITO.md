# xdialog in this fork

This directory is the `xdialog` crate, version 4.1.2, as published on crates.io (upstream:
https://github.com/velopack/xdialog, commit e7772b01),
with the changes below. The workspace builds against it through `[patch.crates-io]` in the root
`Cargo.toml`.

The first commit that adds this directory holds the unchanged crate; every change after it is a commit
of its own, so `git log -- src/xdialog` lists exactly what differs from upstream.

## Changes

- A dialog with a progress bar always has the full dialog width. Upstream sizes every dialog to its
  longest line of text, so a progress dialog whose text changes while it is open changed its width
  with every update.

## Updating

When the workspace moves to a newer xdialog: replace this directory with the new crate from crates.io
(without `Cargo.lock`, `.cargo`, `.cargo_vcs_info.json`, `.gitignore`), commit that alone, then apply
the changes listed above again and check the list is still complete.
