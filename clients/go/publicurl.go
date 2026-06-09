package weedforge

import (
	"net/url"
	"strconv"
)

// render builds the query suffix for image params (numeric/enum values only, so
// no injection surface). Returns "" when no params are set.
func (p *ImageParams) render() string {
	if p == nil {
		return ""
	}
	q := url.Values{}
	if p.Width != 0 {
		q.Set("width", strconv.FormatUint(uint64(p.Width), 10))
	}
	if p.Height != 0 {
		q.Set("height", strconv.FormatUint(uint64(p.Height), 10))
	}
	if p.Mode != "" {
		q.Set("mode", string(p.Mode))
	}
	if len(q) == 0 {
		return ""
	}
	return "?" + q.Encode()
}

// buildPublicURL assembles a public URL for a file from a known location. It
// prepends a scheme when the location lacks one (the fix for the Rust core's
// scheme-less public_url bug) and is shared by both the live and pre-fetched paths.
func buildPublicURL(loc VolumeLocation, fid FileId, opts PublicURLOptions) string {
	base := loc.URL
	if opts.PreferPublic && loc.PublicURL != "" {
		base = loc.PublicURL
	}
	return normalizeVolumeURL(base) + "/" + fid.Render() + opts.ImageParams.render()
}
