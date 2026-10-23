# Third-party notices

The root [Apache-2.0 license](LICENSE) covers ContainerHub's original work. It
does not replace the licenses of dependencies, copied icons or other third-party
material. Apple container and Docker are external runtimes, not bundled software.

## Included icons

The prototype's `.agents/design/icons.js` uses Lucide paths under ISC, including
Feather-derived icons under MIT. Both notices are retained in
[LUCIDE-LICENSE.txt](.agents/design/LUCIDE-LICENSE.txt).

The desktop uses assets supplied by `gpui-kit`; its Lucide/Feather icon terms
also apply. The blue ContainerHub artwork is project-specific artwork, not an
Apple or Docker icon. Its source images and design history are in
[the brand directory](.agents/design/brand/README.md).

## Rust dependencies

[Cargo.lock](Cargo.lock) pins dependency versions and registry checksums.
[docs/DEPENDENCIES.md](docs/DEPENDENCIES.md) records package names, versions,
declared licenses and source links for the resolved dependency graph, including
platform-specific and build dependencies. It is an inventory, not a claim that
all dependencies use Apache-2.0 or that every package is linked into each binary.

Notable components include:

| Component | License | Source |
| --- | --- | --- |
| gpui-kit | Apache-2.0 | https://github.com/longbridge/gpui-kit |
| GPUI (`gpui-pre` packages) | Apache-2.0 | https://github.com/zed-industries/zed |
| Tokio, Serde | MIT OR Apache-2.0 | Their pinned crates.io source archives |
| snow, RustCrypto P-256 | Apache-2.0 OR MIT | Their pinned crates.io source archives |
| option-ext, cbindgen, dwrote | MPL-2.0 | Their pinned crates.io source archives |
| Unicode data | Unicode-3.0 | Their pinned crates.io source archives |
| webpki root certificate data | CDLA-Permissive-2.0 | Their pinned crates.io source archives |

For multi-license `OR` expressions, a permitted alternative may be selected;
`AND` expressions require all listed terms. MPL-covered files retain MPL-2.0.
Their unmodified source can be obtained from the exact source archive linked in
the inventory; modifications to covered files must be made available under MPL
when required by its terms. No dependency source is vendored or relicensed here.

Run `cargo deny --locked --all-features check licenses` to check the declared
licenses against `deny.toml`. Regenerate the inventory with
`python3 scripts/dependency-licenses.py`. Before distributing compiled products,
collect the exact build's license texts, copyright notices and any required
corresponding source; the inventory alone does not fulfill those obligations.

This repository currently publishes source, not notarized application or npm
releases. Local development packaging is described in the README.
