package io.weedforge;

import java.io.UnsupportedEncodingException;
import java.net.URLEncoder;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ThreadLocalRandom;
import java.util.concurrent.atomic.AtomicLong;

/**
 * HA-aware master adapter. Loops over every configured master on failure, bounded by
 * maxRetries (always at least one full pass — fixing the Rust core's max_retries==0
 * footgun), with linear backoff between rounds.
 */
final class MasterClient {

    private final HttpTransport transport;
    private final List<String> baseUrls;
    private final MasterSelectionStrategy strategy;
    private final int rounds;
    private final long backoffMillis;
    private final AtomicLong counter = new AtomicLong(0);

    MasterClient(HttpTransport transport, List<String> baseUrls, MasterSelectionStrategy strategy,
                 int maxRetries, long backoffMillis) {
        this.transport = transport;
        this.baseUrls = baseUrls;
        this.strategy = strategy;
        this.rounds = Math.max(1, maxRetries);
        this.backoffMillis = backoffMillis;
    }

    static final class AssignResult {
        final FileId fileId;
        final String url;
        final String publicUrl;
        final long count;

        AssignResult(FileId fileId, String url, String publicUrl, long count) {
            this.fileId = fileId;
            this.url = url;
            this.publicUrl = publicUrl;
            this.count = count;
        }
    }

    private int startIndex() {
        int n = baseUrls.size();
        switch (strategy) {
            case FAILOVER:
                return 0;
            case RANDOM:
                return ThreadLocalRandom.current().nextInt(n);
            case ROUND_ROBIN:
            default:
                long v = counter.getAndIncrement();
                return (int) (Math.floorMod(v, (long) n));
        }
    }

    AssignResult assign(AssignOptions opts) {
        int n = baseUrls.size();
        int start = startIndex();
        String last = null;
        for (int round = 0; round < rounds; round++) {
            for (int offset = 0; offset < n; offset++) {
                String base = baseUrls.get((start + offset) % n);
                try {
                    return assignOne(base, opts);
                } catch (WeedException e) {
                    last = e.getMessage();
                }
            }
            sleepBackoff(round);
        }
        throw new WeedException.AllMastersUnavailable(last);
    }

    LookupResult lookup(long volumeId) {
        int n = baseUrls.size();
        int start = startIndex();
        String last = null;
        for (int round = 0; round < rounds; round++) {
            for (int offset = 0; offset < n; offset++) {
                String base = baseUrls.get((start + offset) % n);
                try {
                    return lookupOne(base, volumeId);
                } catch (WeedException e) {
                    last = e.getMessage();
                }
            }
            sleepBackoff(round);
        }
        throw new WeedException.AllMastersUnavailable(last);
    }

    private AssignResult assignOne(String base, AssignOptions opts) {
        StringBuilder q = new StringBuilder();
        appendParam(q, "replication", opts.replication);
        appendParam(q, "dataCenter", opts.dataCenter);
        appendParam(q, "rack", opts.rack);
        appendParam(q, "ttl", opts.ttl);
        appendParam(q, "collection", opts.collection);
        String endpoint = base + "/dir/assign" + (q.length() > 0 ? "?" + q : "");

        HttpTransport.Response resp = sendGet(endpoint, "assignment");
        if (resp.statusCode / 100 != 2) {
            throw new WeedException.AssignmentFailed("HTTP status: " + resp.statusCode);
        }
        Map<String, Object> obj;
        try {
            obj = Json.asObject(Json.parse(resp.bodyAsString()));
        } catch (RuntimeException e) {
            throw new WeedException.AssignmentFailed("failed to parse response: " + e.getMessage());
        }
        String error = Json.optString(obj, "error");
        if (error != null && !error.isEmpty()) {
            throw new WeedException.AssignmentFailed(error);
        }
        String fidStr = Json.optString(obj, "fid");
        if (fidStr == null) {
            throw new WeedException.AssignmentFailed("response missing fid");
        }
        FileId fid = FileId.parse(fidStr);
        long count = Json.optLong(obj, "count", 1);
        if (count == 0) count = 1;
        return new AssignResult(fid, Json.optString(obj, "url"), Json.optString(obj, "publicUrl"), count);
    }

    private LookupResult lookupOne(String base, long volumeId) {
        String endpoint = base + "/dir/lookup?volumeId=" + volumeId;
        HttpTransport.Response resp = sendGet(endpoint, "lookup");
        if (resp.statusCode / 100 != 2) {
            throw new WeedException.DownloadFailed("lookup HTTP status: " + resp.statusCode);
        }
        Map<String, Object> obj;
        try {
            obj = Json.asObject(Json.parse(resp.bodyAsString()));
        } catch (RuntimeException e) {
            throw new WeedException.DownloadFailed("failed to parse lookup response: " + e.getMessage());
        }
        String error = Json.optString(obj, "error");
        if (error != null && !error.isEmpty()) {
            throw new WeedException.VolumeNotFound(volumeId);
        }
        List<LookupResult.VolumeLocation> locs = new ArrayList<LookupResult.VolumeLocation>();
        List<Object> arr = Json.optArray(obj, "locations");
        if (arr != null) {
            for (Object o : arr) {
                Map<String, Object> loc = Json.asObject(o);
                locs.add(new LookupResult.VolumeLocation(
                        Json.optString(loc, "url"), Json.optString(loc, "publicUrl")));
            }
        }
        return new LookupResult(volumeId, locs);
    }

    private HttpTransport.Response sendGet(String endpoint, String op) {
        try {
            return transport.send(HttpTransport.Request.get(endpoint));
        } catch (Exception e) {
            // Transport failure is retriable; preserve the cause.
            if ("assignment".equals(op)) {
                throw new WeedException.AssignmentFailed("HTTP request failed: " + e.getMessage());
            }
            throw new WeedException.DownloadFailed(op + " request failed: " + e.getMessage());
        }
    }

    private void sleepBackoff(int round) {
        if (backoffMillis <= 0) return;
        try {
            Thread.sleep(backoffMillis * (round + 1));
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }
    }

    private static void appendParam(StringBuilder q, String key, String value) {
        if (value == null || value.isEmpty()) return;
        if (q.length() > 0) q.append('&');
        q.append(key).append('=').append(encode(value)); // percent-encode — fixes injection
    }

    private static String encode(String v) {
        try {
            return URLEncoder.encode(v, "UTF-8");
        } catch (UnsupportedEncodingException e) {
            throw new IllegalStateException(e); // UTF-8 always present
        }
    }
}
