package weedforge

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strconv"
	"testing"
)

type vectorsFile struct {
	Valid []struct {
		Input    string `json:"input"`
		VolumeID uint32 `json:"volume_id"`
		FileKey  string `json:"file_key"` // decimal string (full unsigned 64-bit range)
		Cookie   uint32 `json:"cookie"`
		Render   string `json:"render"`
		Note     string `json:"note"`
	} `json:"valid"`
	Invalid []struct {
		Input  string `json:"input"`
		Reason string `json:"reason"`
	} `json:"invalid"`
}

func loadVectors(t *testing.T) vectorsFile {
	t.Helper()
	path := filepath.Join("..", "conformance", "fileid_vectors.json")
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read vectors: %v", err)
	}
	var v vectorsFile
	if err := json.Unmarshal(raw, &v); err != nil {
		t.Fatalf("parse vectors: %v", err)
	}
	return v
}

func TestConformanceValid(t *testing.T) {
	v := loadVectors(t)
	if len(v.Valid) == 0 {
		t.Fatal("no valid vectors loaded")
	}
	for _, vec := range v.Valid {
		fid, err := ParseFileID(vec.Input)
		if err != nil {
			t.Errorf("parse(%q) unexpected error: %v", vec.Input, err)
			continue
		}
		wantKey, perr := strconv.ParseUint(vec.FileKey, 10, 64)
		if perr != nil {
			t.Fatalf("bad file_key in vector %q: %v", vec.Input, perr)
		}
		if fid.VolumeID != vec.VolumeID || fid.FileKey != wantKey || fid.Cookie != vec.Cookie {
			t.Errorf("parse(%q) = {vol:%d key:%d cookie:%d}, want {vol:%d key:%d cookie:%d}",
				vec.Input, fid.VolumeID, fid.FileKey, fid.Cookie, vec.VolumeID, wantKey, vec.Cookie)
		}
		if got := fid.Render(); got != vec.Render {
			t.Errorf("render(parse(%q)) = %q, want %q", vec.Input, got, vec.Render)
		}
		// Render is canonical and idempotent under re-parse.
		reparsed, err := ParseFileID(fid.Render())
		if err != nil {
			t.Errorf("re-parse(%q) error: %v", fid.Render(), err)
			continue
		}
		if reparsed != fid {
			t.Errorf("parse->render->parse not stable for %q: %+v vs %+v", vec.Input, reparsed, fid)
		}
	}
}

func TestConformanceInvalid(t *testing.T) {
	v := loadVectors(t)
	for _, vec := range v.Invalid {
		if _, err := ParseFileID(vec.Input); err == nil {
			t.Errorf("parse(%q) expected error (%s), got nil", vec.Input, vec.Reason)
		}
	}
}

func TestRenderMatchesSeaweedCanonical(t *testing.T) {
	// Large needle key (> 2^32) that the Rust core silently truncates; verify the
	// full 64-bit value survives round-trip.
	fid := NewFileId(7, 0x1_0000_0001, 0xDEADBEEF)
	const want = "7,0100000001deadbeef"
	if got := fid.Render(); got != want {
		t.Fatalf("render = %q, want %q", got, want)
	}
	back, err := ParseFileID(want)
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}
	if back != fid {
		t.Fatalf("round-trip mismatch: %+v vs %+v", back, fid)
	}
}
