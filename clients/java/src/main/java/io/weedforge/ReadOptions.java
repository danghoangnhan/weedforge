package io.weedforge;

/** Options for a read. */
public final class ReadOptions {
    ReplicaSelection replicaSelection = ReplicaSelection.STICKY;

    public ReadOptions replicaSelection(ReplicaSelection v) {
        this.replicaSelection = v;
        return this;
    }
}
