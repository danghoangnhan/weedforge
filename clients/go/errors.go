package weedforge

import "fmt"

// InvalidFileIDError indicates a malformed fid string. Mirrors DomainError::InvalidFileId.
type InvalidFileIDError struct {
	Value  string
	Reason string
}

func (e *InvalidFileIDError) Error() string {
	return fmt.Sprintf("invalid file ID %q: %s", e.Value, e.Reason)
}

// VolumeNotFoundError indicates the master could not resolve a volume id.
type VolumeNotFoundError struct{ VolumeID uint32 }

func (e *VolumeNotFoundError) Error() string {
	return fmt.Sprintf("volume %d not found", e.VolumeID)
}

// NoReplicasAvailableError indicates a lookup returned no volume locations.
type NoReplicasAvailableError struct{ VolumeID uint32 }

func (e *NoReplicasAvailableError) Error() string {
	return fmt.Sprintf("no replicas available for volume %d", e.VolumeID)
}

// FileNotFoundError indicates a 404 from a volume server.
type FileNotFoundError struct{ FileID string }

func (e *FileNotFoundError) Error() string {
	return fmt.Sprintf("file not found: %s", e.FileID)
}

// AssignmentFailedError wraps an assign failure.
type AssignmentFailedError struct{ Reason string }

func (e *AssignmentFailedError) Error() string { return "assignment failed: " + e.Reason }

// UploadFailedError wraps an upload failure.
type UploadFailedError struct{ Reason string }

func (e *UploadFailedError) Error() string { return "upload failed: " + e.Reason }

// DownloadFailedError wraps a download failure.
type DownloadFailedError struct{ Reason string }

func (e *DownloadFailedError) Error() string { return "download failed: " + e.Reason }

// DeleteFailedError wraps a delete failure. (The Rust core mislabels these as
// DownloadFailed; this client uses a dedicated type.)
type DeleteFailedError struct{ Reason string }

func (e *DeleteFailedError) Error() string { return "delete failed: " + e.Reason }

// AllMastersUnavailableError indicates every configured master failed.
type AllMastersUnavailableError struct{ Last error }

func (e *AllMastersUnavailableError) Error() string {
	if e.Last != nil {
		return "all masters unavailable: last error: " + e.Last.Error()
	}
	return "all masters unavailable"
}

func (e *AllMastersUnavailableError) Unwrap() error { return e.Last }

// ConfigurationError indicates invalid client configuration.
type ConfigurationError struct{ Reason string }

func (e *ConfigurationError) Error() string { return "configuration error: " + e.Reason }
