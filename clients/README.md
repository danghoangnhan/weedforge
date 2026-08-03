# weedforge native clients (Go & Java)

Pure-language SeaweedFS clients that mirror the weedforge Rust/Python SDK's API and
semantics **without binding the Rust core**. This is the "native thin client" approach:
chosen because the core is small (a handful of HTTP calls + `FileId` encoding + an HA
retry loop), SeaweedFS itself is Go, and binding would re-import the native
cross-compilation/packaging tax for no real gain — especially for Java on **JDK 8**,
where the modern FFI path (Project Panama / FFM, JDK 22+) is unavailable.

```
clients/
  go/             pure-Go client (stdlib only, CGO_ENABLED=0)
  java/           pure-Java client (JDK 8+, zero runtime deps, HttpURLConnection)
  conformance/    shared cross-language test vectors (FileId parse/render)
```

## Parity is enforced, not assumed

**All three** implementations run the same golden vectors in
`conformance/fileid_vectors.json` — Go and Java from their own test suites, Rust from
`tests/conformance.rs`. Until that Rust test existed the claim in this heading was not
quite true: the reference implementation the other two are kept in parity *with* was the
one nothing checked against the shared file.

The `FileId` codec is implemented to SeaweedFS's exact wire format:

> fid = `{volumeId},{hex}` where `hex` is the 12-byte big-endian buffer
> `[needleKey(8) | cookie(4)]` with leading zero **bytes** stripped. `parse()` takes the
> last 8 hex chars as the cookie and the remaining prefix as the (up to 64-bit) needle key.

This is **correct for the full 64-bit needle-key range** and **byte-stable** with the
server.

### Divergences, and where they stand

These clients were written ahead of the Rust core on six points. All six have since been
fixed upstream, so the table below is history rather than a warning:

| Former Rust core bug | Resolution |
|---|---|
| `FileId` packed key+cookie into one u64, rejecting/truncating keys > 2³² | fixed in the core; full 64-bit key everywhere |
| `public_url()` omitted the URL scheme | fixed in the core; `http://` prepended when scheme-less |
| `max_retries = 0` (via `Default`) never contacted a master | fixed in the core; retries clamp to ≥ 1 full pass, and `Default` now equals `new()` |
| `/dir/assign` params not URL-encoded | fixed in the core via reqwest's `query()`; `url.Values` / `URLEncoder` here |
| unbounded download body read (DoS) | fixed in the core via `HttpClientConfig::max_download_bytes`; `MaxDownloadBytes` here |
| delete errors mislabeled `DownloadFailed` | fixed in the core with a dedicated `DeleteFailed` |

**The core is now ahead of these clients on one point:** a read resolves to a single
replica here, and does not fall back to another when that server is unreachable. The Rust
core tries every replica the master returned. Under a rack-aware replication code such as
`010` that is the difference between surviving the loss of a storage node and not, so it
is the next thing to port in this direction.

## Go

```bash
cd clients/go
go test ./...                               # unit + conformance tests
CGO_ENABLED=0 GOOS=linux GOARCH=arm64 go build ./...   # free cross-compile, no native artifacts
```

```go
c, _ := weedforge.NewBuilder().MasterURL("http://master1:9333").Build()
fid, _ := c.Write(ctx, []byte("hello"), "hello.txt")
data, _ := c.Read(ctx, fid)
url, _ := c.PublicURL(ctx, fid)
_ = c.Delete(ctx, fid)
```

The HTTP seam is the `Doer` interface (`*http.Client` satisfies it); inject a fake in tests.

## Java (JDK 8+)

```bash
cd clients/java
mvn test
```

```java
WeedClient c = WeedClient.builder().masterUrl("http://master1:9333").build();
FileId fid = c.write("hello".getBytes(StandardCharsets.UTF_8), "hello.txt");
byte[] data = c.read(fid);
String url = c.publicUrl(fid);
c.delete(fid);
```

Zero runtime dependencies: HTTP via `java.net.HttpURLConnection` behind a pluggable
`HttpTransport` interface (swap in OkHttp/Apache, or a test fake), JSON hand-rolled.
JDK 8 constraints honored: no records/`var`, unsigned values via `Long.parseUnsignedLong`/
`>>>`, `CompletableFuture`-friendly (blocking calls are safe to wrap in your own executor).

## Status

Both are built and tested by `.github/workflows/clients.yml`, which also triggers on
`src/**` — the core is where a conformance-breaking change is most likely to originate.

- **Go**: `go vet`/`gofmt` clean, all tests pass, cross-compiles to linux/amd64,
  linux/arm64, windows/amd64 and darwin/arm64 with `CGO_ENABLED=0`.
- **Java**: built and tested on JDK 8, 11 and 17 (`mvn -B -ntp verify`). The shared
  conformance vectors hold `FileId` in parity with Go and with the Rust core.

Neither client has been exercised against a live SeaweedFS server; only the Rust and
Python surfaces are, by the `integration` job in `.github/workflows/ci.yml`.
