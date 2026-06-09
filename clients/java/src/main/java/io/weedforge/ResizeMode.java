package io.weedforge;

/** Image resize mode for public URLs. */
public enum ResizeMode {
    /** Resize to fit within the dimensions, preserving aspect ratio. */
    FIT("fit"),
    /** Resize to fill the dimensions, cropping if necessary. */
    FILL("fill");

    private final String wire;

    ResizeMode(String wire) { this.wire = wire; }

    public String wireName() { return wire; }
}
