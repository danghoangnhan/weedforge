package io.weedforge;

/** Options for a write, built fluently. All fields optional. */
public final class WriteOptions {
    String filename;
    String contentType;
    String replication;
    String dataCenter;
    String collection;
    String ttl;

    public WriteOptions filename(String v) { this.filename = v; return this; }
    public WriteOptions contentType(String v) { this.contentType = v; return this; }
    public WriteOptions replication(String v) { this.replication = v; return this; }
    public WriteOptions dataCenter(String v) { this.dataCenter = v; return this; }
    public WriteOptions collection(String v) { this.collection = v; return this; }
    public WriteOptions ttl(String v) { this.ttl = v; return this; }
}
