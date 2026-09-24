Rouille 3.6.2 depends on `multipart` 0.18 with only its `server` feature enabled.
That release and its `buf_redux` dependency produce Rust future compatibility
warnings because their macros expand to trailing semicolons in expressions.

This adapter re-exports the pinned `multipart_ng` 0.18.0 release under the package
name Rouille expects. It also uses `buf_redux_ng` 0.8.4 through that release's
dependencies. Both registry packages and their checksums are pinned in the root
`Cargo.lock`.

The published Rust sources were compared with the original crates: the only
source changes remove the trailing semicolons in `invalid_cont_disp!`,
`do_read!`, and `flush_amt!`. The manifests rename the packages and point to the
patched buffer dependency. No HTTP parsing logic changes are required.

Sources:

- https://github.com/CarloTUM/multipart_ng
- https://github.com/CarloTUM/buf_redux_ng

Remove this adapter and the root `[patch.crates-io]` entry when upgrading to a
Rouille release that includes compatible multipart dependencies.
