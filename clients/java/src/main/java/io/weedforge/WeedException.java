package io.weedforge;

/**
 * Base type for all weedforge errors (unchecked). Concrete subtypes mirror the Rust
 * core's {@code DomainError} variants so callers can {@code catch} specific failures.
 */
public class WeedException extends RuntimeException {

    public WeedException(String message) { super(message); }

    public WeedException(String message, Throwable cause) { super(message, cause); }

    /** The fid string is malformed. */
    public static final class InvalidFileId extends WeedException {
        public InvalidFileId(String value, String reason) {
            super("invalid file ID '" + value + "': " + reason);
        }
    }

    /** The master could not resolve a volume id. */
    public static final class VolumeNotFound extends WeedException {
        public VolumeNotFound(long volumeId) { super("volume " + volumeId + " not found"); }
    }

    /** A lookup returned no replica locations. */
    public static final class NoReplicasAvailable extends WeedException {
        public NoReplicasAvailable(long volumeId) {
            super("no replicas available for volume " + volumeId);
        }
    }

    /** A volume server returned 404 for the file. */
    public static final class FileNotFound extends WeedException {
        public FileNotFound(String fileId) { super("file not found: " + fileId); }
    }

    /** Assignment failed. */
    public static final class AssignmentFailed extends WeedException {
        public AssignmentFailed(String reason) { super("assignment failed: " + reason); }
    }

    /** Upload failed. */
    public static final class UploadFailed extends WeedException {
        public UploadFailed(String reason) { super("upload failed: " + reason); }
    }

    /** Download failed. */
    public static final class DownloadFailed extends WeedException {
        public DownloadFailed(String reason) { super("download failed: " + reason); }
    }

    /** Delete failed. (The Rust core mislabels these as DownloadFailed.) */
    public static final class DeleteFailed extends WeedException {
        public DeleteFailed(String reason) { super("delete failed: " + reason); }
    }

    /** Every configured master failed. */
    public static final class AllMastersUnavailable extends WeedException {
        public AllMastersUnavailable(String lastError) {
            super(lastError == null
                    ? "all masters unavailable"
                    : "all masters unavailable: last error: " + lastError);
        }
    }

    /** Invalid client configuration. */
    public static final class Configuration extends WeedException {
        public Configuration(String reason) { super("configuration error: " + reason); }
    }
}
