package io.weedforge;

import java.util.HashMap;
import java.util.Map;

/** Adapter for volume-server upload/download/delete. */
final class VolumeClient {

    private final HttpTransport transport;
    private final long maxDownloadBytes;

    VolumeClient(HttpTransport transport, long maxDownloadBytes) {
        this.transport = transport;
        this.maxDownloadBytes = maxDownloadBytes;
    }

    static final class UploadResult {
        final FileId fileId;
        final long size;
        final String etag;

        UploadResult(FileId fileId, long size, String etag) {
            this.fileId = fileId;
            this.size = size;
            this.etag = etag;
        }
    }

    private static String buildFileUrl(String base, FileId fid) {
        return Urls.normalizeVolume(base) + "/" + fid.render();
    }

    UploadResult upload(String base, FileId fid, byte[] data, String filename, String contentType) {
        if (filename == null || filename.isEmpty()) filename = "file";
        if (contentType == null || contentType.isEmpty()) contentType = "application/octet-stream";

        Multipart mp = Multipart.singleFile(data, filename, contentType);
        Map<String, String> headers = new HashMap<String, String>();
        headers.put("Content-Type", mp.contentType);
        HttpTransport.Request req = new HttpTransport.Request("POST", buildFileUrl(base, fid), headers, mp.body);

        HttpTransport.Response resp;
        try {
            resp = transport.send(req);
        } catch (Exception e) {
            throw new WeedException.UploadFailed("HTTP request failed: " + e.getMessage());
        }
        if (resp.statusCode / 100 != 2) {
            throw new WeedException.UploadFailed("HTTP status: " + resp.statusCode);
        }
        Map<String, Object> obj;
        try {
            obj = Json.asObject(Json.parse(resp.bodyAsString()));
        } catch (RuntimeException e) {
            throw new WeedException.UploadFailed("failed to parse response: " + e.getMessage());
        }
        String error = Json.optString(obj, "error");
        if (error != null && !error.isEmpty()) {
            throw new WeedException.UploadFailed(error);
        }
        return new UploadResult(fid, Json.optLong(obj, "size", 0), Json.optString(obj, "eTag"));
    }

    byte[] download(String base, FileId fid) {
        HttpTransport.Response resp;
        try {
            resp = transport.send(HttpTransport.Request.get(buildFileUrl(base, fid)));
        } catch (Exception e) {
            throw new WeedException.DownloadFailed("HTTP request failed: " + e.getMessage());
        }
        if (resp.statusCode == 404) {
            throw new WeedException.FileNotFound(fid.render());
        }
        if (resp.statusCode / 100 != 2) {
            throw new WeedException.DownloadFailed("HTTP status: " + resp.statusCode);
        }
        if (maxDownloadBytes > 0 && resp.body.length > maxDownloadBytes) {
            throw new WeedException.DownloadFailed("response body exceeds maxDownloadBytes");
        }
        return resp.body;
    }

    void delete(String base, FileId fid) {
        HttpTransport.Response resp;
        try {
            resp = transport.send(HttpTransport.Request.delete(buildFileUrl(base, fid)));
        } catch (Exception e) {
            throw new WeedException.DeleteFailed("HTTP request failed: " + e.getMessage());
        }
        if (resp.statusCode == 404) {
            throw new WeedException.FileNotFound(fid.render());
        }
        if (resp.statusCode / 100 != 2) {
            throw new WeedException.DeleteFailed("HTTP status: " + resp.statusCode);
        }
    }
}
