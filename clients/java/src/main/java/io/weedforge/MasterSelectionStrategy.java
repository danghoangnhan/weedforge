package io.weedforge;

/** Strategy for choosing which master a request starts from. */
public enum MasterSelectionStrategy {
    /** Distribute requests across masters in rotation. */
    ROUND_ROBIN("round_robin"),
    /** Always start at the first master, falling over only on error. */
    FAILOVER("failover"),
    /** Start from a random master each request. */
    RANDOM("random");

    private final String wire;

    MasterSelectionStrategy(String wire) { this.wire = wire; }

    /** @return the lowercase wire name (matching the Rust/Python SDK). */
    public String wireName() { return wire; }

    /** Parses a wire name; throws {@link WeedException.Configuration} if unknown. */
    public static MasterSelectionStrategy fromWire(String s) {
        for (MasterSelectionStrategy v : values()) {
            if (v.wire.equals(s)) return v;
        }
        throw new WeedException.Configuration(
                "invalid strategy: " + s + ". Must be 'round_robin', 'failover', or 'random'");
    }
}
