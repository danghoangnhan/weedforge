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

Both clients run the **same** golden vectors in `conformance/fileid_vectors.json`. The
`FileId` codec is implemented to SeaweedFS's exact wire format:

> fid = `{volumeId},{hex}` where `hex` is the 12-byte big-endian buffer
> `[needleKey(8) | cookie(4)]` with leading zero **bytes** stripped. `parse()` takes the
> last 8 hex chars as the cookie and the remaining prefix as the (up to 64-bit) needle key.

This is **correct for the full 64-bit needle-key range** and **byte-stable** with the
server — it deliberately does *not* reproduce the Rust core's current bugs:

| Rust core bug (see review) | Fixed here |
|---|---|
| `FileId` packs key+cookie into one u64, rejects/truncates keys > 2³² | full 64-bit key, round-trips losslessly |
| `public_url()` omits the URL scheme | volume/public URLs get `http://` prepended when scheme-less |
| `max_retries = 0` (via `Default`) never contacts a master | retries clamped to ≥ 1 full pass |
| `/dir/assign` params not URL-encoded | percent-encoded via `url.Values` / `URLEncoder` |
| unbounded download body read (DoS) | optional `MaxDownloadBytes` cap |
| delete errors mislabeled `DownloadFailed` | dedicated `DeleteFailed` |

> Note: the upstream Rust core still has these bugs. Fix them there too so the reference
> and the clients converge (see the code-review report).

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
mvn test          # requires a JDK + Maven (not available in this sandbox)
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

- **Go**: compiles, `go vet`/`gofmt` clean, all tests pass, cross-compiles to
  linux/arm64, windows/amd64, darwin/arm64 with `CGO_ENABLED=0`.
- **Java**: written to JDK 8; **not yet compiled in CI** (no JVM in the authoring
  environment). Run `mvn test` to verify; the shared conformance vectors guarantee
  `FileId` parity with the Go reference.
