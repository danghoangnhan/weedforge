package weedforge

import (
	"context"
	"encoding/json"
	"io"
	"math/rand"
	"net/http"
	"net/url"
	"strconv"
	"sync/atomic"
	"time"
)

// AssignResult is the master's response to a file-id assignment.
type AssignResult struct {
	FileID    FileId
	URL       string // internal volume URL to upload to
	PublicURL string // optional public volume URL
	Count     int
}

// VolumeLocation is one replica location for a volume.
type VolumeLocation struct {
	URL       string
	PublicURL string
}

// LookupResult is the master's response to a volume lookup.
type LookupResult struct {
	VolumeID  uint32
	Locations []VolumeLocation
}

type assignResponse struct {
	Fid       string `json:"fid"`
	URL       string `json:"url"`
	PublicURL string `json:"publicUrl"`
	Count     int    `json:"count"`
	Error     string `json:"error"`
}

type lookupResponse struct {
	VolumeID  string             `json:"volumeId"`
	Locations []locationResponse `json:"locations"`
	Error     string             `json:"error"`
}

type locationResponse struct {
	URL       string `json:"url"`
	PublicURL string `json:"publicUrl"`
}

// masterClient is the HA-aware master adapter. It loops over every configured
// master on failure and bounds the attempts by maxRetries (always at least one
// full pass — fixing the Rust core's max_retries==0 footgun), with linear backoff
// between rounds.
type masterClient struct {
	doer       Doer
	baseURLs   []string
	strategy   MasterSelectionStrategy
	maxRetries int
	backoff    time.Duration
	counter    uint64 // atomic, for round-robin
}

func (m *masterClient) rounds() int {
	if m.maxRetries < 1 {
		return 1
	}
	return m.maxRetries
}

func (m *masterClient) startIndex() int {
	n := len(m.baseURLs)
	switch m.strategy {
	case Failover:
		return 0
	case Random:
		return rand.Intn(n)
	default: // RoundRobin
		return int(atomic.AddUint64(&m.counter, 1)-1) % n
	}
}

// assign tries each master, returning on the first success.
func (m *masterClient) assign(ctx context.Context, opts AssignOptions) (AssignResult, error) {
	n := len(m.baseURLs)
	var last error = &AllMastersUnavailableError{}
	start := m.startIndex()
	for round := 0; round < m.rounds(); round++ {
		for offset := 0; offset < n; offset++ {
			base := m.baseURLs[(start+offset)%n]
			res, err := m.assignOne(ctx, base, opts)
			if err == nil {
				return res, nil
			}
			last = err
		}
		m.sleepBackoff(ctx, round)
	}
	return AssignResult{}, &AllMastersUnavailableError{Last: last}
}

func (m *masterClient) lookup(ctx context.Context, volumeID uint32) (LookupResult, error) {
	n := len(m.baseURLs)
	var last error = &AllMastersUnavailableError{}
	start := m.startIndex()
	for round := 0; round < m.rounds(); round++ {
		for offset := 0; offset < n; offset++ {
			base := m.baseURLs[(start+offset)%n]
			res, err := m.lookupOne(ctx, base, volumeID)
			if err == nil {
				return res, nil
			}
			last = err
		}
		m.sleepBackoff(ctx, round)
	}
	return LookupResult{}, &AllMastersUnavailableError{Last: last}
}

func (m *masterClient) sleepBackoff(ctx context.Context, round int) {
	if m.backoff <= 0 {
		return
	}
	d := time.Duration(round+1) * m.backoff
	t := time.NewTimer(d)
	defer t.Stop()
	select {
	case <-ctx.Done():
	case <-t.C:
	}
}

func (m *masterClient) assignOne(ctx context.Context, base string, opts AssignOptions) (AssignResult, error) {
	q := url.Values{}
	if opts.Replication != "" {
		q.Set("replication", opts.Replication)
	}
	if opts.DataCenter != "" {
		q.Set("dataCenter", opts.DataCenter)
	}
	if opts.Rack != "" {
		q.Set("rack", opts.Rack)
	}
	if opts.TTL != "" {
		q.Set("ttl", opts.TTL)
	}
	if opts.Collection != "" {
		q.Set("collection", opts.Collection)
	}
	endpoint := base + "/dir/assign"
	if len(q) > 0 {
		endpoint += "?" + q.Encode() // url.Values.Encode percent-encodes — fixes injection
	}

	resp, err := m.get(ctx, endpoint)
	if err != nil {
		return AssignResult{}, &AssignmentFailedError{Reason: "HTTP request failed: " + err.Error()}
	}
	defer drain(resp)

	if resp.StatusCode/100 != 2 {
		return AssignResult{}, &AssignmentFailedError{Reason: "HTTP status: " + resp.Status}
	}
	var ar assignResponse
	if err := json.NewDecoder(resp.Body).Decode(&ar); err != nil {
		return AssignResult{}, &AssignmentFailedError{Reason: "failed to parse response: " + err.Error()}
	}
	if ar.Error != "" {
		return AssignResult{}, &AssignmentFailedError{Reason: ar.Error}
	}
	fid, err := ParseFileID(ar.Fid)
	if err != nil {
		return AssignResult{}, err
	}
	count := ar.Count
	if count == 0 {
		count = 1
	}
	return AssignResult{FileID: fid, URL: ar.URL, PublicURL: ar.PublicURL, Count: count}, nil
}

func (m *masterClient) lookupOne(ctx context.Context, base string, volumeID uint32) (LookupResult, error) {
	endpoint := base + "/dir/lookup?volumeId=" + strconv.FormatUint(uint64(volumeID), 10)
	resp, err := m.get(ctx, endpoint)
	if err != nil {
		// Transport failure is retriable; preserve the cause rather than collapsing
		// to VolumeNotFound (the Rust core's lossy behavior).
		return LookupResult{}, &DownloadFailedError{Reason: "lookup request failed: " + err.Error()}
	}
	defer drain(resp)

	if resp.StatusCode/100 != 2 {
		return LookupResult{}, &DownloadFailedError{Reason: "lookup HTTP status: " + resp.Status}
	}
	var lr lookupResponse
	if err := json.NewDecoder(resp.Body).Decode(&lr); err != nil {
		return LookupResult{}, &DownloadFailedError{Reason: "failed to parse lookup response: " + err.Error()}
	}
	if lr.Error != "" {
		return LookupResult{}, &VolumeNotFoundError{VolumeID: volumeID}
	}
	locs := make([]VolumeLocation, 0, len(lr.Locations))
	for _, l := range lr.Locations {
		locs = append(locs, VolumeLocation{URL: l.URL, PublicURL: l.PublicURL})
	}
	return LookupResult{VolumeID: volumeID, Locations: locs}, nil
}

func (m *masterClient) get(ctx context.Context, endpoint string) (*http.Response, error) {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, endpoint, nil)
	if err != nil {
		return nil, err
	}
	return m.doer.Do(req)
}

// drain closes a response body (reading the small remainder so the connection can
// be reused by the keep-alive pool).
func drain(resp *http.Response) {
	if resp == nil || resp.Body == nil {
		return
	}
	_, _ = io.Copy(io.Discard, io.LimitReader(resp.Body, 4<<10))
	_ = resp.Body.Close()
}
