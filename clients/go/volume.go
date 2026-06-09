package weedforge

import (
	"bytes"
	"context"
	"encoding/json"
	"io"
	"mime/multipart"
	"net/http"
	"net/textproto"
	"strings"
)

// UploadResult is the volume server's response to an upload.
type UploadResult struct {
	FileID FileId
	Size   int64
	ETag   string
}

type uploadResponse struct {
	Size  int64  `json:"size"`
	ETag  string `json:"eTag"`
	Error string `json:"error"`
}

type volumeClient struct {
	doer             Doer
	maxDownloadBytes int64
}

// buildFileURL composes the volume request URL, normalizing the scheme.
func buildFileURL(base string, fid FileId) string {
	return normalizeVolumeURL(base) + "/" + fid.Render()
}

func (v *volumeClient) upload(ctx context.Context, base string, fid FileId, data []byte, filename, contentType string) (UploadResult, error) {
	if filename == "" {
		filename = "file"
	}
	if contentType == "" {
		contentType = "application/octet-stream"
	}

	var body bytes.Buffer
	w := multipart.NewWriter(&body)
	h := make(textproto.MIMEHeader)
	h.Set("Content-Disposition", `form-data; name="file"; filename="`+escapeQuotes(filename)+`"`)
	h.Set("Content-Type", contentType)
	part, err := w.CreatePart(h)
	if err != nil {
		return UploadResult{}, &UploadFailedError{Reason: "failed to build multipart: " + err.Error()}
	}
	if _, err := part.Write(data); err != nil {
		return UploadResult{}, &UploadFailedError{Reason: "failed to write multipart: " + err.Error()}
	}
	if err := w.Close(); err != nil {
		return UploadResult{}, &UploadFailedError{Reason: "failed to close multipart: " + err.Error()}
	}

	req, err := http.NewRequestWithContext(ctx, http.MethodPost, buildFileURL(base, fid), &body)
	if err != nil {
		return UploadResult{}, &UploadFailedError{Reason: err.Error()}
	}
	req.Header.Set("Content-Type", w.FormDataContentType())

	resp, err := v.doer.Do(req)
	if err != nil {
		return UploadResult{}, &UploadFailedError{Reason: "HTTP request failed: " + err.Error()}
	}
	defer drain(resp)
	if resp.StatusCode/100 != 2 {
		return UploadResult{}, &UploadFailedError{Reason: "HTTP status: " + resp.Status}
	}
	var ur uploadResponse
	if err := json.NewDecoder(resp.Body).Decode(&ur); err != nil {
		return UploadResult{}, &UploadFailedError{Reason: "failed to parse response: " + err.Error()}
	}
	if ur.Error != "" {
		return UploadResult{}, &UploadFailedError{Reason: ur.Error}
	}
	return UploadResult{FileID: fid, Size: ur.Size, ETag: ur.ETag}, nil
}

func (v *volumeClient) download(ctx context.Context, base string, fid FileId) ([]byte, error) {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, buildFileURL(base, fid), nil)
	if err != nil {
		return nil, &DownloadFailedError{Reason: err.Error()}
	}
	resp, err := v.doer.Do(req)
	if err != nil {
		return nil, &DownloadFailedError{Reason: "HTTP request failed: " + err.Error()}
	}
	defer drain(resp)
	if resp.StatusCode == http.StatusNotFound {
		return nil, &FileNotFoundError{FileID: fid.Render()}
	}
	if resp.StatusCode/100 != 2 {
		return nil, &DownloadFailedError{Reason: "HTTP status: " + resp.Status}
	}

	var reader io.Reader = resp.Body
	if v.maxDownloadBytes > 0 {
		// Read one extra byte so we can detect (rather than silently truncate)
		// a body that exceeds the cap.
		reader = io.LimitReader(resp.Body, v.maxDownloadBytes+1)
	}
	data, err := io.ReadAll(reader)
	if err != nil {
		return nil, &DownloadFailedError{Reason: "failed to read response body: " + err.Error()}
	}
	if v.maxDownloadBytes > 0 && int64(len(data)) > v.maxDownloadBytes {
		return nil, &DownloadFailedError{Reason: "response body exceeds MaxDownloadBytes"}
	}
	return data, nil
}

func (v *volumeClient) delete(ctx context.Context, base string, fid FileId) error {
	req, err := http.NewRequestWithContext(ctx, http.MethodDelete, buildFileURL(base, fid), nil)
	if err != nil {
		return &DeleteFailedError{Reason: err.Error()}
	}
	resp, err := v.doer.Do(req)
	if err != nil {
		return &DeleteFailedError{Reason: "HTTP request failed: " + err.Error()}
	}
	defer drain(resp)
	if resp.StatusCode == http.StatusNotFound {
		return &FileNotFoundError{FileID: fid.Render()}
	}
	if resp.StatusCode/100 != 2 {
		return &DeleteFailedError{Reason: "HTTP status: " + resp.Status}
	}
	return nil
}

func escapeQuotes(s string) string {
	return strings.NewReplacer("\\", "\\\\", `"`, `\"`).Replace(s)
}
