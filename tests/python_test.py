"""Python tests for weedforge.

These tests require a running SeaweedFS instance.
Run with: pytest tests/python_test.py -v
"""

import os
import pytest

# Skip all tests if weedforge is not installed
pytest.importorskip("weedforge")

from weedforge import WeedClient, FileId


class TestFileId:
    """Tests for FileId class."""

    def test_parse_valid(self):
        """Test parsing a valid file ID."""
        fid = FileId.parse("3,0000016300007037")
        assert fid.volume_id == 3
        assert isinstance(fid.file_key, int)
        assert isinstance(fid.cookie, int)

    def test_parse_invalid(self):
        """Test parsing an invalid file ID."""
        with pytest.raises(RuntimeError):
            FileId.parse("invalid")

    def test_render(self):
        """Test rendering a file ID."""
        fid = FileId.parse("3,0000016300007037")
        rendered = fid.render()
        assert rendered.startswith("3,")

    def test_str(self):
        """Test string representation."""
        fid = FileId.parse("3,0000016300007037")
        assert str(fid).startswith("3,")

    def test_repr(self):
        """Test repr."""
        fid = FileId.parse("3,0000016300007037")
        assert "FileId" in repr(fid)
        assert "volume_id=" in repr(fid)


class TestWeedClient:
    """Tests for WeedClient class."""

    def test_create_client(self):
        """Test creating a client."""
        client = WeedClient(master_urls=["http://localhost:9333"])
        assert client is not None

    def test_create_client_with_strategy(self):
        """Test creating a client with different strategies."""
        for strategy in ["round_robin", "failover", "random"]:
            client = WeedClient(
                master_urls=["http://localhost:9333"],
                strategy=strategy,
            )
            assert client is not None

    def test_create_client_invalid_strategy(self):
        """Test creating a client with invalid strategy."""
        with pytest.raises(RuntimeError):
            WeedClient(
                master_urls=["http://localhost:9333"],
                strategy="invalid",
            )

    def test_parse_file_id(self):
        """Test static parse_file_id method."""
        fid = WeedClient.parse_file_id("3,0000016300007037")
        assert fid.volume_id == 3


@pytest.mark.skipif(
    not os.environ.get("SEAWEEDFS_MASTER"),
    reason="SEAWEEDFS_MASTER not set",
)
class TestIntegration:
    """Integration tests requiring a running SeaweedFS instance."""

    @pytest.fixture
    def client(self):
        """Create a test client."""
        master_url = os.environ.get("SEAWEEDFS_MASTER", "http://localhost:9333")
        return WeedClient(master_urls=[master_url])

    def test_write_read_delete(self, client):
        """Test write, read, and delete operations."""
        # Write
        data = b"Hello, SeaweedFS!"
        fid = client.write(data, filename="test.txt")
        assert fid is not None
        assert fid.volume_id > 0

        # Read
        downloaded = client.read(fid)
        assert downloaded == data

        # Read with string fid
        downloaded2 = client.read(str(fid))
        assert downloaded2 == data

        # Public URL
        url = client.public_url(fid)
        assert url.startswith("http")

        # Delete
        client.delete(fid)

    def test_upload_bytes_alias(self, client):
        """Test upload_bytes alias."""
        data = b"Test data"
        fid = client.upload_bytes(data, filename="alias_test.txt")
        assert fid is not None

        # Cleanup
        client.delete(fid)

    def test_public_url_resized(self, client):
        """Test public URL with resize parameters."""
        data = b"fake image data"
        fid = client.write(data, filename="image.jpg")

        url = client.public_url_resized(fid, width=200, height=200)
        assert "width=200" in url
        assert "height=200" in url

        # Cleanup
        client.delete(fid)


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
