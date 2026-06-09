package weedforge

import "time"

// MasterSelectionStrategy controls which master a request starts from.
type MasterSelectionStrategy string

const (
	// RoundRobin distributes requests across masters in rotation.
	RoundRobin MasterSelectionStrategy = "round_robin"
	// Failover always starts at the first master, falling over only on error.
	Failover MasterSelectionStrategy = "failover"
	// Random starts from a random master each request.
	Random MasterSelectionStrategy = "random"
)

// ReplicaSelection controls which replica a read targets.
type ReplicaSelection int

const (
	// ReplicaSticky deterministically maps a file to one replica (stable per file).
	ReplicaSticky ReplicaSelection = iota
	// ReplicaFirst always reads from the first available replica.
	ReplicaFirst
)

// ResizeMode is an image resize mode for public URLs.
type ResizeMode string

const (
	// ResizeFit resizes to fit within the dimensions, preserving aspect ratio.
	ResizeFit ResizeMode = "fit"
	// ResizeFill resizes to fill the dimensions, cropping if necessary.
	ResizeFill ResizeMode = "fill"
)

// AssignOptions are parameters for a file-id assignment.
type AssignOptions struct {
	Replication string
	DataCenter  string
	Rack        string
	TTL         string
	Collection  string
}

// WriteOptions configure a write.
type WriteOptions struct {
	Filename    string
	ContentType string
	Replication string
	DataCenter  string
	Collection  string
	TTL         string
}

// ReadOptions configure a read.
type ReadOptions struct {
	ReplicaSelection ReplicaSelection
}

// ImageParams describe an image transformation appended to a public URL.
type ImageParams struct {
	Width  uint32 // 0 = unset
	Height uint32 // 0 = unset
	Mode   ResizeMode
}

// PublicURLOptions configure public-URL construction.
type PublicURLOptions struct {
	ImageParams  *ImageParams
	PreferPublic bool
}

// Config tunes the underlying HTTP behavior.
type Config struct {
	ConnectTimeout     time.Duration
	RequestTimeout     time.Duration
	InsecureSkipVerify bool
	// MaxDownloadBytes caps a single download to avoid memory-exhaustion from a
	// hostile/buggy volume server. 0 means unlimited.
	MaxDownloadBytes int64
	// Backoff is the base delay between retry rounds (with linear growth). 0 disables.
	Backoff time.Duration
}

// DefaultConfig returns sensible defaults.
func DefaultConfig() Config {
	return Config{
		ConnectTimeout:     5 * time.Second,
		RequestTimeout:     30 * time.Second,
		InsecureSkipVerify: false,
		MaxDownloadBytes:   0,
		Backoff:            50 * time.Millisecond,
	}
}
