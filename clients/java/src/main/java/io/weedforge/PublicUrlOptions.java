package io.weedforge;

/** Options for public-URL construction. */
public final class PublicUrlOptions {
    ImageParams imageParams; // nullable
    boolean preferPublic;

    public PublicUrlOptions imageParams(ImageParams v) { this.imageParams = v; return this; }
    public PublicUrlOptions preferPublic(boolean v) { this.preferPublic = v; return this; }
}
