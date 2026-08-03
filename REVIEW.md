# weedforge — code review

Reviewed at `danghoangnhan/weedforge@main` (crate 0.1.1, PyPI 0.1.1) against the
`ha_seaweedfs` deployment it is recommended for in `docs/clients.md`.

That deployment is the yardstick used for severity below: 3 Raft masters on
`10.88.0.21-23:9333`, `-defaultReplication=010` so every object has exactly two
copies in **different racks**, volume servers on `10.88.0.2x:8080` with no
`-publicUrl`, and NGINX deliberately not proxying volume servers — so a native-fid
client talks straight to volume servers and must be a mesh member.

---

## A. Must fix — these break the cluster's advertised behaviour

### A1. No read failover across replicas

`src/application/read_file.rs` — `ReadFileUseCase::execute`

```rust
let source_url = lookup.locations[location_index].url.clone();
let data = self.volume.download(&source_url, file_id).await?;
```

One location is chosen and downloaded from. If that volume server is down the read
fails, even though `lookup` just returned a live second replica.

`010` exists precisely so one host can die. `docs/operations.md:110` promises "One
storage VM dies → served by remaining two". On the S3 path that holds. On the native
path this SDK does not deliver it.

**Fix:** iterate the locations, trying the next on transport failure / 5xx / 404,
and return the last error only when all replicas fail.

**Note:** the Go client has the same gap (`clients/go/client.go` `ReadWithOptions`),
so this is a design fix in both, not a port.

### A2. `ReplicaSelection::Random` is neither random nor well distributed

`src/application/read_file.rs`

```rust
fn simple_hash(file_id: &FileId) -> usize {
    let combined =
        (u64::from(file_id.volume_id()) << 32) | (file_id.file_key() ^ u64::from(file_id.cookie()));
    combined as usize
}
```

Three problems, compounding:

1. It is **deterministic per object**, not random per request. A hot object is pinned
   to one replica forever and read load never spreads across the two racks.
2. It is the **default** (`#[default] Random` on `ReplicaSelection`), so plain
   `client.read(&fid)` gets this behaviour without opting in.
3. With two replicas, `combined % 2` reduces to the low bit of `file_key ^ cookie` —
   `volume_id << 32` contributes nothing modulo 2. Volume identity does not
   participate in the choice at all.

`docs/clients.md:83-89` in ha_seaweedfs already warns, in the abstract, about an SDK
whose "random" strategy is really a hash of the file id. The SDK that page recommends
is the one that has it.

**Fix:** rename to `Sticky` (honest, and matches the Go client's `ReplicaSticky`), use
a real hash (Go uses FNV-64a over all 16 bytes), and add a genuinely per-request
`Random` variant. Renaming the variant is **API-breaking**.

### A3. `AssignOptions.rack` is never sent

`src/domain/ports.rs` declares it:

```rust
pub struct AssignOptions {
    pub replication: Option<String>,
    pub data_center: Option<String>,
    pub rack: Option<String>,      // <- declared
    ...
}
```

`src/infrastructure/http/master.rs` — `assign_impl` sends `replication`, `dataCenter`,
`ttl`, `collection`. Not `rack`. And `WriteOptions` (`src/application/write_file.rs`)
has no `rack` field at all, so a caller cannot set it even in principle.

On a cluster whose entire failure-domain model is racks (`rack-b`/`rack-c`/`rack-d`,
one per host), rack-targeted placement is simply unavailable.

**Fix:** send `rack` in `assign_impl`; add `rack` to `WriteOptions` and thread it
through `WriteFileUseCase::execute`. The Go client already does this
(`clients/go/master.go`, `q.Set("rack", opts.Rack)`) — straight port.

### A4. Assign query parameters are not URL-encoded

`src/infrastructure/http/master.rs` — `assign_impl`

```rust
params.push(format!("collection={collection}"));
...
url.push_str(&params.join("&"));
```

Values go into the query string raw. A value containing `&`, `=`, `#` or a space
injects parameters or truncates the URL.

**Concrete failure:** `collection = "logs&replication=000"` produces
`/dir/assign?collection=logs&replication=000`, silently writing an **unreplicated**
object to a cluster whose whole design is two-rack replication. Nothing errors.

**Fix:** percent-encode. Go uses `url.Values.Encode()`; the Rust equivalent is
building with `reqwest`'s `.query(&[...])` — straight port.

---

## B. Worth fixing

### B1. `lookup_impl` collapses every failure into `VolumeNotFound`, poisoning HA state

`src/infrastructure/http/master.rs` maps transport error, non-2xx **and** JSON parse
error all to `DomainError::VolumeNotFound`. `src/infrastructure/ha.rs` then treats that
as a *master* failure:

```rust
Err(e) => { last_error = e; self.mark_failed(index).await; }
```

So a genuinely missing volume costs `max_retries(3) × masters(3)` = **9 requests**
before failing, and marks all three healthy masters as failed on the way. The caller
also cannot distinguish "volume gone" from "network down".

Go's `lookupOne` preserves the cause and only returns `VolumeNotFoundError` when the
master actually populated its `error` field — straight port.

### B2. `failed_masters` is dead state on the hot path

`src/infrastructure/ha.rs` maintains

```rust
failed_masters: Arc<RwLock<Vec<usize>>>,
```

written by `mark_success` / `mark_failed` on every request. `next_master_index()`
never consults it, and nothing else in the crate reads it. Every request takes an
async write lock to maintain a list with no effect on behaviour.

**Fix:** delete it, or actually use it (skip known-bad masters with periodic
re-probing). Deleting is the honest fix.

### B3. Downloads are unbounded

`src/infrastructure/http/volume.rs` — `download_impl` calls `response.bytes().await`
with no cap, and there is no streaming alternative. `HttpClientConfig` has no size
knob. A hostile or buggy volume server allocates the whole body in RAM; so does a
legitimately large object (`VOLUME_SIZE_LIMIT_MB=30000` on this cluster).

Go has `Config.MaxDownloadBytes`. Port it, and add a streaming read for large objects.

### B4. Delete failures are reported as `DownloadFailed`

`src/infrastructure/http/volume.rs` — `delete_impl` returns
`DomainError::DownloadFailed` on both transport and non-2xx failure. `DomainError` has
no delete variant at all, so a failed delete is indistinguishable from a failed read
in logs and in `match` arms.

**Fix:** add `DomainError::DeleteFailed`. Adding an enum variant is **API-breaking**
for any caller matching exhaustively.

### B5. `public_url()` and `public_url_resized()` return different hosts

`src/client.rs`:

- `public_url` → `build(file_id, None)` → `PublicUrlOptions::default()` →
  `prefer_public: false` → uses `location.url` (internal).
- `public_url_resized` → explicitly `prefer_public: true` → uses `location.public_url`
  when present.

Same file, two methods, two different hosts. On ha_seaweedfs `-publicUrl` is unset so
both collapse to the overlay address and the bug is invisible — it is latent for any
deployment that sets it.

**Fix:** make them consistent and let the caller choose.

### B6. Delete and public_url always use `locations[0]`

`src/application/delete_file.rs` and `src/application/public_url.rs` both hardcode
`locations[0]`. If that replica's host is down, delete fails and `public_url` hands out
a dead URL. Same failover treatment as A1 applies.

### B7. `BlockingWeedClient` panics inside an async runtime

`src/client.rs` — `build_blocking` creates its own `tokio::runtime::Runtime` and every
method calls `runtime.block_on`. Calling any of them from inside an existing Tokio
runtime panics with *"Cannot start a runtime from within a runtime"*. Reaching for the
blocking client from an axum/actix handler is a natural mistake.

**Fix:** detect with `Handle::try_current()` and return `ConfigurationError`, or at
minimum document it on the type.

### B8. `Builder::default()` and `Builder::new()` disagree

`WeedClientBuilder` derives `Default` (giving `max_retries: 0`) but `new()` sets `3`.
Currently harmless — `ha.rs` clamps with `.max(1)` — but the two constructors silently
differ. Same for `HaMasterClientBuilder`.

**Fix:** `impl Default { fn default() -> Self { Self::new() } }`.

### B9. No auth support

There is no way to set request headers, so SeaweedFS's JWT
(`-jwt.signing.key` on master and volume) cannot be used. Not needed on ha_seaweedfs
today — the WireGuard mesh is the security boundary — but it blocks any deployment
that enables it.

---

## C. Python binding

The package README calls this "A lightweight Python SDK for SeaweedFS", so Python is
a first-class surface.

### C1. The GIL is never released during network I/O — the biggest issue in the crate

`src/python/mod.rs` — every `#[pymethods]` method on `PyWeedClient` calls straight
into `BlockingWeedClient`, which does `runtime.block_on(...)`:

```rust
fn write(&self, data: &[u8], filename: Option<&str>) -> PyResult<PyFileId> {
    let file_id = self.client.write(data.to_vec(), filename).map_err(to_py_err)?;
    Ok(PyFileId { inner: file_id })
}
```

No `py.allow_threads` anywhere in the file. The GIL is held for the entire network
round trip, so two Python threads uploading concurrently serialize completely. A
`ThreadPoolExecutor` over this client gets no overlap at all — the one thing a native
extension is supposed to buy you.

**Fix:** wrap every blocking call in `Python::allow_threads`.

### C2. One exception type for everything

`to_py_err` maps every `DomainError` to a bare `PyRuntimeError`. A caller cannot
distinguish "file not found" from "all masters unavailable" from "invalid file id"
without string-matching `str(e)`.

**Fix:** a module exception hierarchy — `WeedError` base with `FileNotFound`,
`AllMastersUnavailable`, `InvalidFileId`, `UploadFailed`, … subclasses.

### C3. Most of the API is unreachable from Python

Exposed: `write`, `upload_bytes`, `read`, `delete`, `public_url`,
`public_url_resized`, `parse_file_id`.

Missing versus `BlockingWeedClient`: **`lookup`**, `write_with_options`,
`read_with_options`, and all of `HttpClientConfig` (connect/request timeouts).

`lookup` matters concretely here: without it a Python caller cannot see how many
replicas an object has or which hosts hold them — exactly the check an operator on
this cluster wants to make, and the one assertion that proves `010` placement worked.

### C4. No type stubs

No `.pyi` and no `py.typed` marker, so every Python caller gets `Any` and no editor
completion. For an abi3 extension module this is the only way to get types at all.

---

## D. Documentation is stale in ways that mislead

### D1. `clients/README.md`'s bug table is wrong on half its rows

It asserts *"the upstream Rust core still has these bugs"*. Checked against current
`src/`:

| Claimed Rust core bug | Actual state |
|---|---|
| `FileId` packs key+cookie into one u64, rejects/truncates keys > 2³² | **fixed** — `entities.rs` uses `file_key: u64` and a 12-byte BE buffer; `test_max_needle_key` covers `u64::MAX` |
| `public_url()` omits the URL scheme | **fixed** — `assemble_url` prepends `http://`, with a test |
| `max_retries = 0` never contacts a master | **fixed** — `ha.rs` uses `.max(1)`; `max_retries_zero_still_attempts_once` covers it |
| `/dir/assign` params not URL-encoded | still real — see A4 |
| unbounded download body read | still real — see B3 |
| delete errors mislabeled `DownloadFailed` | still real — see B4 |

Three of six are fixed. A document telling readers the reference implementation is
broken when it is not is itself a defect.

### D2. The Java CI claim is stale

`clients/README.md` says the Java client is *"not yet compiled in CI (no JVM in the
authoring environment)"*. `.github/workflows/clients.yml` builds and tests it on JDK
8, 11 and 17.

### D3. "Parity is enforced, not assumed" is false for the reference implementation

`clients/conformance/fileid_vectors.json` is loaded by `FileIdConformanceTest.java`
and the Go tests. **No Rust test loads it.** The implementation that Go and Java are
supposed to be in parity *with* is the one not checked against the shared vectors.

**Fix:** add a Rust test that reads the same JSON.

### D4. The conformance workflow is not triggered by the code that breaks conformance

`.github/workflows/clients.yml` triggers only on `paths: clients/**`. A change to
`src/domain/entities.rs` that alters the FileId wire format never runs the Go/Java
conformance tests. Add `src/domain/entities.rs` (or just `src/**`) to the trigger.

---

## E. CI — the SDK has never talked to a real server

- `tests/integration.rs::test_write_read_delete` is `#[ignore]` and gated on
  `SEAWEEDFS_MASTER`.
- `tests/python_test.py::TestIntegration` is `skipif(not os.environ.get("SEAWEEDFS_MASTER"))`.
- `SEAWEEDFS_MASTER` is set in **no** workflow, and no workflow starts a SeaweedFS
  service or container.

So `cargo test --all-features` runs unit tests only, and the entire
assign → upload → lookup → download → delete path has never been exercised in CI.
`release.yml` publishes to crates.io and PyPI without it.

Every defect in section A is invisible to the current test suite and would have been
caught by one run against a real cluster.

---

## Breaking-change summary

These need a version decision (0.1.1 → 0.2.0):

- A2 — renaming `ReplicaSelection::Random` → `Sticky`
- B4 — adding `DomainError::DeleteFailed` (breaks exhaustive matches)
- A3 — adding a field to `WriteOptions` (breaks struct literals without `..Default::default()`)

Non-breaking: A1, A4, B1, B2, B3, B5, B6, B7, B8, C1–C4, D1–D4, E.
