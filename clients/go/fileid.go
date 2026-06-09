package weedforge

import (
	"encoding/binary"
	"encoding/hex"
	"strconv"
	"strings"
)

// FileId is a SeaweedFS file identifier: a first-class value, not an opaque string.
//
// The wire form is "{volumeId},{hex}" where hex encodes the 12-byte big-endian
// buffer [needleKey(8) | cookie(4)] with leading zero BYTES stripped. The needle
// key is a full 64-bit value (NOT 32-bit), so this type represents the entire
// SeaweedFS fid space.
type FileId struct {
	VolumeID uint32
	FileKey  uint64
	Cookie   uint32
}

// NewFileId builds a FileId from its components.
func NewFileId(volumeID uint32, fileKey uint64, cookie uint32) FileId {
	return FileId{VolumeID: volumeID, FileKey: fileKey, Cookie: cookie}
}

const (
	cookieHexLen = 8  // cookie is 4 bytes -> 8 hex chars
	maxKCHexLen  = 24 // 8-byte key + 4-byte cookie -> 24 hex chars
)

// ParseFileID parses a SeaweedFS fid string. It mirrors SeaweedFS's own parser:
// the last 8 hex characters are the cookie and the remaining prefix is the
// (up to 64-bit) needle key.
func ParseFileID(s string) (FileId, error) {
	comma := strings.IndexByte(s, ',')
	if comma < 0 {
		return FileId{}, &InvalidFileIDError{Value: s, Reason: "missing comma separator"}
	}
	volStr, kc := s[:comma], s[comma+1:]

	if !isAllDigits(volStr) {
		return FileId{}, &InvalidFileIDError{Value: s, Reason: "invalid volume id"}
	}
	vol, err := strconv.ParseUint(volStr, 10, 32)
	if err != nil {
		return FileId{}, &InvalidFileIDError{Value: s, Reason: "invalid volume id: " + err.Error()}
	}

	// SeaweedFS requires len(kc) > 8 (must carry at least the cookie plus one key
	// nibble) and <= 24. Rejecting all non-hex bytes here also rejects '+'/'-' signs
	// that strconv.ParseUint would otherwise silently accept.
	if len(kc) <= cookieHexLen || len(kc) > maxKCHexLen {
		return FileId{}, &InvalidFileIDError{Value: s, Reason: "key/cookie hex length out of range"}
	}
	if !isAllHex(kc) {
		return FileId{}, &InvalidFileIDError{Value: s, Reason: "non-hex character in key/cookie"}
	}

	split := len(kc) - cookieHexLen
	keyHex, cookieHex := kc[:split], kc[split:]

	key, err := strconv.ParseUint(keyHex, 16, 64)
	if err != nil {
		return FileId{}, &InvalidFileIDError{Value: s, Reason: "invalid needle key: " + err.Error()}
	}
	cookie, err := strconv.ParseUint(cookieHex, 16, 32)
	if err != nil {
		return FileId{}, &InvalidFileIDError{Value: s, Reason: "invalid cookie: " + err.Error()}
	}

	return FileId{VolumeID: uint32(vol), FileKey: key, Cookie: uint32(cookie)}, nil
}

// Render returns the canonical SeaweedFS string form (leading zero bytes stripped).
// Render(Parse(s)) reproduces the server's representation; Parse(Render(f)) == f.
func (f FileId) Render() string {
	var buf [12]byte
	binary.BigEndian.PutUint64(buf[0:8], f.FileKey)
	binary.BigEndian.PutUint32(buf[8:12], f.Cookie)

	// Strip leading zero bytes, keeping at least the final byte (so the all-zero
	// null fid renders as "00" rather than empty).
	i := 0
	for i < len(buf)-1 && buf[i] == 0 {
		i++
	}
	return strconv.FormatUint(uint64(f.VolumeID), 10) + "," + hex.EncodeToString(buf[i:])
}

// String implements fmt.Stringer.
func (f FileId) String() string { return f.Render() }

func isAllDigits(s string) bool {
	if s == "" {
		return false
	}
	for i := 0; i < len(s); i++ {
		if s[i] < '0' || s[i] > '9' {
			return false
		}
	}
	return true
}

func isAllHex(s string) bool {
	if s == "" {
		return false
	}
	for i := 0; i < len(s); i++ {
		c := s[i]
		isHex := (c >= '0' && c <= '9') || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F')
		if !isHex {
			return false
		}
	}
	return true
}
