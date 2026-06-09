package weedforge

import (
	"context"
	"io"
	"net/http"
	"strings"
	"sync/atomic"
	"testing"
)

// fakeDoer routes requests to a user-supplied handler.
type fakeDoer struct {
	handle func(*http.Request) (*http.Response, error)
}

func (f *fakeDoer) Do(req *http.Request) (*http.Response, error) { return f.handle(req) }

func resp(status int, body string) *http.Response {
	return &http.Response{
		StatusCode: status,
		Status:     http.StatusText(status),
		Body:       io.NopCloser(strings.NewReader(body)),
		Header:     make(http.Header),
	}
}

const assignOK = `{"fid":"3,01637037d6","url":"vol1:8080","publicUrl":"pub1:8080","count":1}`
const uploadOK = `{"size":13,"eTag":"abc"}`

func isAssign(r *http.Request) bool { return strings.Contains(r.URL.Path, "/dir/assign") }
func isLookup(r *http.Request) bool { return strings.Contains(r.URL.Path, "/dir/lookup") }

func TestAssignFailoverToSecondMaster(t *testing.T) {
	var firstHits, secondHits int32
	doer := &fakeDoer{handle: func(req *http.Request) (*http.Response, error) {
		if isAssign(req) {
			if strings.Contains(req.URL.Host, "master1") {
				atomic.AddInt32(&firstHits, 1)
				return resp(500, "boom"), nil
			}
			atomic.AddInt32(&secondHits, 1)
			return resp(200, assignOK), nil
		}
		return resp(200, uploadOK), nil // volume upload
	}}

	c, err := NewBuilder().
		MasterURLs("http://master1:9333", "http://master2:9333").
		Strategy(Failover).
		Transport(doer).
		Build()
	if err != nil {
		t.Fatal(err)
	}

	fid, err := c.Write(context.Background(), []byte("hello seaweed"), "x.txt")
	if err != nil {
		t.Fatalf("write with failover failed: %v", err)
	}
	if fid.VolumeID != 3 || fid.FileKey != 1 {
		t.Fatalf("unexpected fid: %+v", fid)
	}
	if atomic.LoadInt32(&firstHits) == 0 || atomic.LoadInt32(&secondHits) == 0 {
		t.Fatalf("expected both masters tried; first=%d second=%d", firstHits, secondHits)
	}
}

func TestMaxRetriesZeroStillTriesOnce(t *testing.T) {
	var assignHits int32
	doer := &fakeDoer{handle: func(req *http.Request) (*http.Response, error) {
		if isAssign(req) {
			atomic.AddInt32(&assignHits, 1)
			return resp(200, assignOK), nil
		}
		return resp(200, uploadOK), nil
	}}
	c, err := NewBuilder().
		MasterURL("http://master1:9333").
		MaxRetries(0). // would mean "never try" in the buggy Rust core
		Transport(doer).
		Build()
	if err != nil {
		t.Fatal(err)
	}
	if _, err := c.Write(context.Background(), []byte("x"), ""); err != nil {
		t.Fatalf("expected one attempt to succeed, got: %v", err)
	}
	if atomic.LoadInt32(&assignHits) != 1 {
		t.Fatalf("expected exactly 1 assign attempt, got %d", assignHits)
	}
}

func TestPublicURLPrependsScheme(t *testing.T) {
	doer := &fakeDoer{handle: func(req *http.Request) (*http.Response, error) {
		return resp(200, `{"volumeId":"3","locations":[{"url":"127.0.0.1:8080","publicUrl":"cdn.example.com"}]}`), nil
	}}
	c, err := NewBuilder().MasterURL("http://master1:9333").Transport(doer).Build()
	if err != nil {
		t.Fatal(err)
	}
	fid, _ := ParseFileID("3,01637037d6")

	url, err := c.PublicURL(context.Background(), fid)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.HasPrefix(url, "http://127.0.0.1:8080/") {
		t.Fatalf("public URL missing scheme/host: %q", url)
	}
	if !strings.HasSuffix(url, "/3,01637037d6") {
		t.Fatalf("public URL missing fid: %q", url)
	}

	rurl, err := c.PublicURLResized(context.Background(), fid, 200, 100)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.HasPrefix(rurl, "http://cdn.example.com/") {
		t.Fatalf("resized URL should prefer public host: %q", rurl)
	}
	if !strings.Contains(rurl, "width=200") || !strings.Contains(rurl, "height=100") {
		t.Fatalf("resized URL missing image params: %q", rurl)
	}
}

func TestReadRoundTripsThroughVolume(t *testing.T) {
	const payload = "hello seaweed"
	doer := &fakeDoer{handle: func(req *http.Request) (*http.Response, error) {
		if isLookup(req) {
			return resp(200, `{"volumeId":"3","locations":[{"url":"vol1:8080"}]}`), nil
		}
		return resp(200, payload), nil // volume download
	}}
	c, err := NewBuilder().MasterURL("http://master1:9333").Transport(doer).Build()
	if err != nil {
		t.Fatal(err)
	}
	fid, _ := ParseFileID("3,01637037d6")
	data, err := c.Read(context.Background(), fid)
	if err != nil {
		t.Fatal(err)
	}
	if string(data) != payload {
		t.Fatalf("read = %q, want %q", string(data), payload)
	}
}

func TestBuildRequiresMasterURL(t *testing.T) {
	if _, err := NewBuilder().Build(); err == nil {
		t.Fatal("expected error when no master URLs configured")
	}
}

func TestAssignEncodesQueryParams(t *testing.T) {
	var seen string
	doer := &fakeDoer{handle: func(req *http.Request) (*http.Response, error) {
		if isAssign(req) {
			seen = req.URL.RawQuery
			return resp(200, assignOK), nil
		}
		return resp(200, uploadOK), nil
	}}
	c, _ := NewBuilder().MasterURL("http://master1:9333").Transport(doer).Build()
	// A collection value containing reserved chars must be percent-encoded, not injected.
	if _, err := c.WriteWithOptions(context.Background(), []byte("x"), WriteOptions{Collection: "a&replication=999"}); err != nil {
		t.Fatalf("write failed: %v", err)
	}
	if !strings.Contains(seen, "collection=a%26replication%3D999") {
		t.Fatalf("query param not percent-encoded: %q", seen)
	}
}

func TestDownloadLimitEnforced(t *testing.T) {
	doer := &fakeDoer{handle: func(req *http.Request) (*http.Response, error) {
		if isLookup(req) {
			return resp(200, `{"volumeId":"3","locations":[{"url":"vol1:8080"}]}`), nil
		}
		return resp(200, strings.Repeat("A", 1000)), nil
	}}
	cfg := DefaultConfig()
	cfg.MaxDownloadBytes = 100
	c, _ := NewBuilder().MasterURL("http://m:9333").Transport(doer).Config(cfg).Build()
	fid, _ := ParseFileID("3,01637037d6")
	if _, err := c.Read(context.Background(), fid); err == nil {
		t.Fatal("expected download-limit error, got nil")
	}
}
