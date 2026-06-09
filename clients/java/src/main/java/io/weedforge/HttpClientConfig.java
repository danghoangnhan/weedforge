package io.weedforge;

/** Tunables for the default {@link HttpUrlConnectionTransport}. */
public final class HttpClientConfig {
    int connectTimeoutMillis = 5_000;
    int requestTimeoutMillis = 30_000;
    boolean insecureSkipVerify = false;
    /** Cap on a single download to avoid memory exhaustion; 0 = unlimited. */
    long maxDownloadBytes = 0;
    /** Base delay (ms) between failover retry rounds, grown linearly; 0 disables. */
    long backoffMillis = 50;

    public HttpClientConfig connectTimeoutMillis(int v) { this.connectTimeoutMillis = v; return this; }
    public HttpClientConfig requestTimeoutMillis(int v) { this.requestTimeoutMillis = v; return this; }
    public HttpClientConfig insecureSkipVerify(boolean v) { this.insecureSkipVerify = v; return this; }
    public HttpClientConfig maxDownloadBytes(long v) { this.maxDownloadBytes = v; return this; }
    public HttpClientConfig backoffMillis(long v) { this.backoffMillis = v; return this; }
}
