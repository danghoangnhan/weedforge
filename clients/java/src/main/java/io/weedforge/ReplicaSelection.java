package io.weedforge;

/** Strategy for choosing which replica a read targets. */
public enum ReplicaSelection {
    /** Deterministically map a file to one replica (stable per file). */
    STICKY,
    /** Always read from the first available replica. */
    FIRST
}
