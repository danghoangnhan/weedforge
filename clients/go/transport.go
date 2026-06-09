package weedforge

import (
	"crypto/tls"
	"net"
	"net/http"
	"strings"
)

// Doer is the minimal HTTP seam the client depends on. *http.Client satisfies it,
// and tests inject fakes to exercise failover/parsing without a live server.
type Doer interface {
	Do(req *http.Request) (*http.Response, error)
}

// newHTTPClient builds a *http.Client honoring the Config (connect timeout via a
// custom dialer, overall request timeout, and TLS verification policy).
func newHTTPClient(cfg Config) *http.Client {
	dialer := &net.Dialer{Timeout: cfg.ConnectTimeout}
	transport := &http.Transport{
		DialContext:         dialer.DialContext,
		TLSClientConfig:     &tls.Config{InsecureSkipVerify: cfg.InsecureSkipVerify}, //nolint:gosec // opt-in only
		MaxIdleConns:        100,
		MaxIdleConnsPerHost: 10,
	}
	return &http.Client{Timeout: cfg.RequestTimeout, Transport: transport}
}

// normalizeMasterURL trims a trailing slash from a master base URL.
func normalizeMasterURL(u string) string {
	return strings.TrimSuffix(u, "/")
}

// normalizeVolumeURL trims a trailing slash and prepends http:// when the volume
// location carries no scheme (SeaweedFS returns bare host:port). This is the fix
// for the Rust core's scheme-less public_url defect, shared by the volume client
// and the public-URL builder.
func normalizeVolumeURL(u string) string {
	u = strings.TrimSuffix(u, "/")
	if !strings.HasPrefix(u, "http://") && !strings.HasPrefix(u, "https://") {
		return "http://" + u
	}
	return u
}
