package io.weedforge;

import java.util.ArrayList;
import java.util.Collection;
import java.util.List;

/**
 * A pure-Java client for SeaweedFS, mirroring the weedforge Rust/Python SDK's API and
 * semantics. JDK 8+ compatible, zero runtime dependencies (HTTP via HttpURLConnection,
 * JSON hand-rolled). FileId encoding, HA master selection, and public-URL construction
 * are kept in parity with the Rust core via the shared conformance vectors under
 * {@code clients/conformance}.
 *
 * <pre>{@code
 * WeedClient client = WeedClient.builder()
 *     .masterUrl("http://master1:9333")
 *     .strategy(MasterSelectionStrategy.ROUND_ROBIN)
 *     .build();
 * FileId fid = client.write("hello".getBytes(StandardCharsets.UTF_8), "hello.txt");
 * byte[] data = client.read(fid);
 * String url = client.publicUrl(fid);
 * client.delete(fid);
 * }</pre>
 */
public final class WeedClient {

    private final MasterClient master;
    private final VolumeClient volume;

    private WeedClient(MasterClient master, VolumeClient volume) {
        this.master = master;
        this.volume = volume;
    }

    public static Builder builder() { return new Builder(); }

    /** Parses a fid string. */
    public static FileId parseFileId(String fid) { return FileId.parse(fid); }

    /** Uploads data, returning its file id. */
    public FileId write(byte[] data, String filename) {
        return writeWithOptions(data, new WriteOptions().filename(filename));
    }

    /** Uploads data with full options. */
    public FileId writeWithOptions(byte[] data, WriteOptions opts) {
        AssignOptions ao = new AssignOptions();
        ao.replication = opts.replication;
        ao.dataCenter = opts.dataCenter;
        ao.collection = opts.collection;
        ao.ttl = opts.ttl;
        MasterClient.AssignResult assign = master.assign(ao);
        VolumeClient.UploadResult up =
                volume.upload(assign.url, assign.fileId, data, opts.filename, opts.contentType);
        return up.fileId;
    }

    /** Downloads a file's bytes. */
    public byte[] read(FileId fid) {
        return readWithOptions(fid, new ReadOptions());
    }

    /** Downloads a file using the given replica-selection strategy. */
    public byte[] readWithOptions(FileId fid, ReadOptions opts) {
        LookupResult lk = master.lookup(fid.volumeId());
        if (lk.locations().isEmpty()) {
            throw new WeedException.NoReplicasAvailable(fid.volumeId());
        }
        int idx = 0;
        if (opts.replicaSelection == ReplicaSelection.STICKY) {
            idx = (int) Math.floorMod(stickyHash(fid), (long) lk.locations().size());
        }
        return volume.download(lk.locations().get(idx).url(), fid);
    }

    /** Removes a file. */
    public void delete(FileId fid) {
        LookupResult lk = master.lookup(fid.volumeId());
        if (lk.locations().isEmpty()) {
            throw new WeedException.NoReplicasAvailable(fid.volumeId());
        }
        volume.delete(lk.locations().get(0).url(), fid);
    }

    /** Returns a public URL for a file. */
    public String publicUrl(FileId fid) {
        return buildPublicUrl(fid, new PublicUrlOptions());
    }

    /** Returns a public URL with image resize parameters. */
    public String publicUrlResized(FileId fid, int width, int height) {
        return buildPublicUrl(fid, new PublicUrlOptions()
                .imageParams(ImageParams.dimensions(width, height))
                .preferPublic(true));
    }

    /** Returns the volume locations for a file id. */
    public LookupResult lookup(FileId fid) {
        return master.lookup(fid.volumeId());
    }

    private String buildPublicUrl(FileId fid, PublicUrlOptions opts) {
        LookupResult lk = master.lookup(fid.volumeId());
        if (lk.locations().isEmpty()) {
            throw new WeedException.NoReplicasAvailable(fid.volumeId());
        }
        LookupResult.VolumeLocation loc = lk.locations().get(0);
        String base = loc.url();
        if (opts.preferPublic && loc.publicUrl() != null && !loc.publicUrl().isEmpty()) {
            base = loc.publicUrl();
        }
        return Urls.normalizeVolume(base) + "/" + fid.render() + imageSuffix(opts.imageParams);
    }

    private static String imageSuffix(ImageParams p) {
        if (p == null) {
            return "";
        }
        List<String> parts = new ArrayList<String>();
        if (p.width != null) parts.add("width=" + p.width);
        if (p.height != null) parts.add("height=" + p.height);
        if (p.mode != null) parts.add("mode=" + p.mode.wireName());
        if (parts.isEmpty()) {
            return "";
        }
        return "?" + String.join("&", parts);
    }

    private static long stickyHash(FileId fid) {
        long h = 1125899906842597L;
        h = 31 * h + fid.volumeId();
        h = 31 * h + fid.fileKey();
        h = 31 * h + fid.cookie();
        return h;
    }

    /** Fluent builder for {@link WeedClient}. */
    public static final class Builder {
        private final List<String> masterUrls = new ArrayList<String>();
        private MasterSelectionStrategy strategy = MasterSelectionStrategy.ROUND_ROBIN;
        private int maxRetries = 3;
        private HttpClientConfig config = new HttpClientConfig();
        private HttpTransport transport; // null -> default HttpURLConnection transport

        public Builder masterUrl(String url) {
            masterUrls.add(url);
            return this;
        }

        public Builder masterUrls(Collection<String> urls) {
            masterUrls.addAll(urls);
            return this;
        }

        public Builder strategy(MasterSelectionStrategy s) {
            this.strategy = s;
            return this;
        }

        public Builder strategy(String wireName) {
            this.strategy = MasterSelectionStrategy.fromWire(wireName);
            return this;
        }

        public Builder maxRetries(int n) {
            this.maxRetries = n;
            return this;
        }

        public Builder config(HttpClientConfig c) {
            this.config = c;
            return this;
        }

        /** Injects a custom transport (e.g. OkHttp/Apache-backed, or a test fake). */
        public Builder transport(HttpTransport t) {
            this.transport = t;
            return this;
        }

        public WeedClient build() {
            if (masterUrls.isEmpty()) {
                throw new WeedException.Configuration("at least one master URL is required");
            }
            HttpTransport t = transport != null ? transport : new HttpUrlConnectionTransport(config);
            List<String> bases = new ArrayList<String>(masterUrls.size());
            for (String u : masterUrls) {
                bases.add(Urls.normalizeMaster(u));
            }
            MasterClient mc = new MasterClient(t, bases, strategy, maxRetries, config.backoffMillis);
            VolumeClient vc = new VolumeClient(t, config.maxDownloadBytes);
            return new WeedClient(mc, vc);
        }
    }
}
