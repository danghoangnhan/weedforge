//! Python bindings for weedforge.
//!
//! This module provides a Python interface to the weedforge `SeaweedFS` SDK.
//! It uses pyo3 to expose Rust functionality to Python.

#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::needless_pass_by_value)]

use crate::client::{BlockingWeedClient, WeedClientBuilder};
use crate::domain::{DomainError, FileId};
use crate::infrastructure::MasterSelectionStrategy;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList};
use std::sync::Arc;

/// Convert a domain error to a Python exception.
fn to_py_err(err: DomainError) -> PyErr {
    PyRuntimeError::new_err(err.to_string())
}

/// A `SeaweedFS` file identifier.
// `from_py_object` keeps the derived `FromPyObject` (used by `PyFileIdOrStr`) now that
// pyo3 0.28 makes it opt-in for `Clone` pyclasses.
#[pyclass(name = "FileId", from_py_object)]
#[derive(Clone)]
pub struct PyFileId {
    inner: FileId,
}

#[pymethods]
impl PyFileId {
    /// Parse a file ID from a string.
    #[staticmethod]
    fn parse(fid: &str) -> PyResult<Self> {
        let inner = FileId::parse(fid).map_err(to_py_err)?;
        Ok(Self { inner })
    }

    /// Returns the volume ID.
    #[getter]
    fn volume_id(&self) -> u32 {
        self.inner.volume_id()
    }

    /// Returns the file key.
    #[getter]
    fn file_key(&self) -> u64 {
        self.inner.file_key()
    }

    /// Returns the cookie value.
    #[getter]
    fn cookie(&self) -> u32 {
        self.inner.cookie()
    }

    /// Renders the file ID as a string.
    fn render(&self) -> String {
        self.inner.render()
    }

    fn __str__(&self) -> String {
        self.inner.render()
    }

    fn __repr__(&self) -> String {
        format!(
            "FileId(volume_id={}, file_key={}, cookie={})",
            self.inner.volume_id(),
            self.inner.file_key(),
            self.inner.cookie()
        )
    }
}

/// `SeaweedFS` client for Python.
#[pyclass(name = "WeedClient")]
pub struct PyWeedClient {
    client: Arc<BlockingWeedClient>,
}

#[pymethods]
impl PyWeedClient {
    /// Create a new `SeaweedFS` client.
    #[new]
    #[pyo3(signature = (master_urls, strategy = "round_robin", max_retries = 3))]
    fn new(master_urls: Vec<String>, strategy: &str, max_retries: usize) -> PyResult<Self> {
        let strategy = match strategy {
            "round_robin" => MasterSelectionStrategy::RoundRobin,
            "failover" => MasterSelectionStrategy::Failover,
            "random" => MasterSelectionStrategy::Random,
            _ => {
                return Err(PyRuntimeError::new_err(format!(
                    "Invalid strategy: {strategy}. Must be 'round_robin', 'failover', or 'random'"
                )))
            }
        };

        let client = WeedClientBuilder::new()
            .master_urls(master_urls)
            .strategy(strategy)
            .max_retries(max_retries)
            .build_blocking()
            .map_err(to_py_err)?;

        Ok(Self {
            client: Arc::new(client),
        })
    }

    /// Write data to `SeaweedFS`.
    #[pyo3(signature = (data, filename = None))]
    fn write(&self, py: Python<'_>, data: &[u8], filename: Option<&str>) -> PyResult<PyFileId> {
        let data = data.to_vec();
        let filename = filename.map(str::to_owned);
        let client = Arc::clone(&self.client);
        let file_id = py
            .detach(move || client.write(data, filename.as_deref()))
            .map_err(to_py_err)?;
        Ok(PyFileId { inner: file_id })
    }

    /// Alias for `write()` - upload bytes to `SeaweedFS`.
    #[pyo3(signature = (data, filename = None))]
    fn upload_bytes(
        &self,
        py: Python<'_>,
        data: &[u8],
        filename: Option<&str>,
    ) -> PyResult<PyFileId> {
        self.write(py, data, filename)
    }

    /// Read data from `SeaweedFS`.
    fn read<'py>(&self, py: Python<'py>, file_id: PyFileIdOrStr) -> PyResult<Bound<'py, PyBytes>> {
        let fid = file_id.into_file_id()?;
        let client = Arc::clone(&self.client);
        let data = py
            .detach(move || client.read(&fid))
            .map_err(to_py_err)?;
        Ok(PyBytes::new(py, &data))
    }

    /// Delete a file from `SeaweedFS`.
    fn delete(&self, py: Python<'_>, file_id: PyFileIdOrStr) -> PyResult<()> {
        let fid = file_id.into_file_id()?;
        let client = Arc::clone(&self.client);
        py.detach(move || client.delete(&fid))
            .map_err(to_py_err)
    }

    /// Get a public URL for a file.
    fn public_url(&self, py: Python<'_>, file_id: PyFileIdOrStr) -> PyResult<String> {
        let fid = file_id.into_file_id()?;
        let client = Arc::clone(&self.client);
        py.detach(move || client.public_url(&fid))
            .map_err(to_py_err)
    }

    /// Get a public URL with image resize parameters.
    fn public_url_resized(
        &self,
        py: Python<'_>,
        file_id: PyFileIdOrStr,
        width: u32,
        height: u32,
    ) -> PyResult<String> {
        let fid = file_id.into_file_id()?;
        let client = Arc::clone(&self.client);
        py.detach(move || client.public_url_resized(&fid, width, height))
            .map_err(to_py_err)
    }

    /// Look up the replica locations for a file id.
    ///
    /// Returns one dict per replica, with `url` and `public_url` keys. This is
    /// how a caller checks that replication actually placed the copies it asked
    /// for -- the Rust API has always had it, and Python simply could not reach
    /// it.
    fn lookup<'py>(
        &self,
        py: Python<'py>,
        file_id: PyFileIdOrStr,
    ) -> PyResult<Bound<'py, PyList>> {
        let fid = file_id.into_file_id()?;
        let client = Arc::clone(&self.client);
        let result = py
            .detach(move || client.lookup(&fid))
            .map_err(to_py_err)?;

        let locations = PyList::empty(py);
        for location in result.locations {
            let entry = PyDict::new(py);
            entry.set_item("url", location.url)?;
            entry.set_item("public_url", location.public_url)?;
            locations.append(entry)?;
        }
        Ok(locations)
    }

    /// Parse a file ID string.
    #[staticmethod]
    fn parse_file_id(fid: &str) -> PyResult<PyFileId> {
        PyFileId::parse(fid)
    }
}

/// Union type for accepting either `FileId` or string.
#[derive(FromPyObject)]
enum PyFileIdOrStr {
    FileId(PyFileId),
    Str(String),
}

impl PyFileIdOrStr {
    fn into_file_id(self) -> PyResult<FileId> {
        match self {
            Self::FileId(py_fid) => Ok(py_fid.inner),
            Self::Str(s) => FileId::parse(&s).map_err(to_py_err),
        }
    }
}

/// Python module definition.
#[pymodule]
pub fn weedforge(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyFileId>()?;
    m.add_class::<PyWeedClient>()?;

    m.add("__doc__", "Rust-first, Python-friendly SDK for `SeaweedFS`")?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    Ok(())
}
