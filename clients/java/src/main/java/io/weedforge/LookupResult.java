package io.weedforge;

import java.util.Collections;
import java.util.List;

/** The master's response to a volume lookup. */
public final class LookupResult {
    private final long volumeId;
    private final List<VolumeLocation> locations;

    public LookupResult(long volumeId, List<VolumeLocation> locations) {
        this.volumeId = volumeId;
        this.locations = Collections.unmodifiableList(locations);
    }

    public long volumeId() { return volumeId; }

    public List<VolumeLocation> locations() { return locations; }

    /** One replica location for a volume. */
    public static final class VolumeLocation {
        private final String url;
        private final String publicUrl; // nullable

        public VolumeLocation(String url, String publicUrl) {
            this.url = url;
            this.publicUrl = publicUrl;
        }

        public String url() { return url; }

        public String publicUrl() { return publicUrl; }
    }
}
