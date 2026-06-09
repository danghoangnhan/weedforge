// Package weedforge is a pure-Go client for SeaweedFS, mirroring the weedforge
// Rust/Python SDK's API and semantics. It has no cgo and no third-party
// dependencies (standard library only), so it cross-compiles for any GOOS/GOARCH.
//
// FileId encoding, HA master selection, and public-URL construction are kept in
// parity with the Rust core via the shared conformance vectors under
// clients/conformance.
package weedforge

import (
	"context"
	"hash/fnv"
)

// WeedClient is a SeaweedFS client.
type WeedClient struct {
	master *masterClient
	volume *volumeClient
}

// Builder configures a WeedClient.
type Builder struct {
	masterURLs []string
	strategy   MasterSelectionStrategy
	maxRetries int
	doer       Doer
	cfg        Config
}

// NewBuilder returns a Builder with defaults (round-robin, 3 retries).
func NewBuilder() *Builder {
	return &Builder{
		strategy:   RoundRobin,
		maxRetries: 3,
		cfg:        DefaultConfig(),
	}
}

// MasterURL adds a single master URL.
func (b *Builder) MasterURL(u string) *Builder { b.masterURLs = append(b.masterURLs, u); return b }

// MasterURLs adds multiple master URLs.
func (b *Builder) MasterURLs(urls ...string) *Builder {
	b.masterURLs = append(b.masterURLs, urls...)
	return b
}

// Strategy sets the master-selection strategy.
func (b *Builder) Strategy(s MasterSelectionStrategy) *Builder { b.strategy = s; return b }

// MaxRetries sets the number of full failover rounds (clamped to at least 1).
func (b *Builder) MaxRetries(n int) *Builder { b.maxRetries = n; return b }

// Config overrides the HTTP configuration.
func (b *Builder) Config(c Config) *Builder { b.cfg = c; return b }

// Transport injects a custom Doer (primarily for testing).
func (b *Builder) Transport(d Doer) *Builder { b.doer = d; return b }

// Build validates configuration and constructs the client.
func (b *Builder) Build() (*WeedClient, error) {
	if len(b.masterURLs) == 0 {
		return nil, &ConfigurationError{Reason: "at least one master URL is required"}
	}
	doer := b.doer
	if doer == nil {
		doer = newHTTPClient(b.cfg)
	}
	bases := make([]string, len(b.masterURLs))
	for i, u := range b.masterURLs {
		bases[i] = normalizeMasterURL(u)
	}
	retries := b.maxRetries
	if retries < 1 {
		retries = 1 // fix the Rust core's max_retries==0 -> never-tries footgun
	}
	return &WeedClient{
		master: &masterClient{
			doer:       doer,
			baseURLs:   bases,
			strategy:   b.strategy,
			maxRetries: retries,
			backoff:    b.cfg.Backoff,
		},
		volume: &volumeClient{doer: doer, maxDownloadBytes: b.cfg.MaxDownloadBytes},
	}, nil
}

// Write uploads data and returns its file id.
func (c *WeedClient) Write(ctx context.Context, data []byte, filename string) (FileId, error) {
	return c.WriteWithOptions(ctx, data, WriteOptions{Filename: filename})
}

// WriteWithOptions uploads data with full options.
func (c *WeedClient) WriteWithOptions(ctx context.Context, data []byte, opts WriteOptions) (FileId, error) {
	assign, err := c.master.assign(ctx, AssignOptions{
		Replication: opts.Replication,
		DataCenter:  opts.DataCenter,
		Collection:  opts.Collection,
		TTL:         opts.TTL,
	})
	if err != nil {
		return FileId{}, err
	}
	res, err := c.volume.upload(ctx, assign.URL, assign.FileID, data, opts.Filename, opts.ContentType)
	if err != nil {
		return FileId{}, err
	}
	return res.FileID, nil
}

// Read downloads a file's bytes.
func (c *WeedClient) Read(ctx context.Context, fid FileId) ([]byte, error) {
	return c.ReadWithOptions(ctx, fid, ReadOptions{})
}

// ReadWithOptions downloads a file using the given replica-selection strategy.
func (c *WeedClient) ReadWithOptions(ctx context.Context, fid FileId, opts ReadOptions) ([]byte, error) {
	lookup, err := c.master.lookup(ctx, fid.VolumeID)
	if err != nil {
		return nil, err
	}
	if len(lookup.Locations) == 0 {
		return nil, &NoReplicasAvailableError{VolumeID: fid.VolumeID}
	}
	idx := 0
	if opts.ReplicaSelection == ReplicaSticky {
		idx = int(stickyHash(fid) % uint64(len(lookup.Locations)))
	}
	return c.volume.download(ctx, lookup.Locations[idx].URL, fid)
}

// Delete removes a file.
func (c *WeedClient) Delete(ctx context.Context, fid FileId) error {
	lookup, err := c.master.lookup(ctx, fid.VolumeID)
	if err != nil {
		return err
	}
	if len(lookup.Locations) == 0 {
		return &NoReplicasAvailableError{VolumeID: fid.VolumeID}
	}
	return c.volume.delete(ctx, lookup.Locations[0].URL, fid)
}

// PublicURL returns a public URL for a file.
func (c *WeedClient) PublicURL(ctx context.Context, fid FileId) (string, error) {
	return c.publicURL(ctx, fid, PublicURLOptions{})
}

// PublicURLResized returns a public URL with image resize parameters.
func (c *WeedClient) PublicURLResized(ctx context.Context, fid FileId, width, height uint32) (string, error) {
	return c.publicURL(ctx, fid, PublicURLOptions{
		ImageParams:  &ImageParams{Width: width, Height: height},
		PreferPublic: true,
	})
}

func (c *WeedClient) publicURL(ctx context.Context, fid FileId, opts PublicURLOptions) (string, error) {
	lookup, err := c.master.lookup(ctx, fid.VolumeID)
	if err != nil {
		return "", err
	}
	if len(lookup.Locations) == 0 {
		return "", &NoReplicasAvailableError{VolumeID: fid.VolumeID}
	}
	return buildPublicURL(lookup.Locations[0], fid, opts), nil
}

// Lookup returns the volume locations for a file id.
func (c *WeedClient) Lookup(ctx context.Context, fid FileId) (LookupResult, error) {
	return c.master.lookup(ctx, fid.VolumeID)
}

// ParseFileID parses a fid string (convenience pass-through).
func ParseFileIDString(s string) (FileId, error) { return ParseFileID(s) }

func stickyHash(fid FileId) uint64 {
	h := fnv.New64a()
	var b [16]byte
	b[0] = byte(fid.VolumeID >> 24)
	b[1] = byte(fid.VolumeID >> 16)
	b[2] = byte(fid.VolumeID >> 8)
	b[3] = byte(fid.VolumeID)
	for i := 0; i < 8; i++ {
		b[4+i] = byte(fid.FileKey >> (56 - 8*i))
	}
	b[12] = byte(fid.Cookie >> 24)
	b[13] = byte(fid.Cookie >> 16)
	b[14] = byte(fid.Cookie >> 8)
	b[15] = byte(fid.Cookie)
	_, _ = h.Write(b[:])
	return h.Sum64()
}
