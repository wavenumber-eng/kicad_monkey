//! Version-bound executable Geometer client for native PCB illustrations.

use std::fmt;
use std::path::{Path, PathBuf};

use geometer_client::contracts::ModelIllustrationGeometryRequestA0;
use geometer_client::{GeometerClient, GeometerClientError, ModelIllustrationGeometry};
use tokio::runtime::Runtime;

pub const GEOMETER_RELEASE: &str = "2026.9.18";
pub const GEOMETER_C_ABI_GENERATION: u32 = 20_260_918;
pub const GEOMETER_SOURCE_REVISION: &str = "68d217e6d133f8712751f6dec9ec9372338055f9";

#[derive(Debug)]
pub enum NativeGeometerError {
    NotFound,
    CurrentExecutable(std::io::Error),
    Runtime(std::io::Error),
    Client(GeometerClientError),
    Incompatible {
        release: String,
        c_abi_generation: u32,
    },
}

impl fmt::Display for NativeGeometerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => formatter.write_str(
                "could not find the compatible Geometer runtime; place it beside kcr or set GEOMETER_EXECUTABLE",
            ),
            Self::CurrentExecutable(error) => {
                write!(formatter, "could not locate the current executable: {error}")
            }
            Self::Runtime(error) => write!(formatter, "could not start the async runtime: {error}"),
            Self::Client(error) => write!(formatter, "Geometer client failed: {error}"),
            Self::Incompatible {
                release,
                c_abi_generation,
            } => write!(
                formatter,
                "Geometer {release} / C ABI {c_abi_generation} is incompatible; expected {GEOMETER_RELEASE} / {GEOMETER_C_ABI_GENERATION}",
            ),
        }
    }
}

impl std::error::Error for NativeGeometerError {}

impl From<GeometerClientError> for NativeGeometerError {
    fn from(error: GeometerClientError) -> Self {
        Self::Client(error)
    }
}

/// One persistent, version-negotiated Geometer process.
///
/// A future bounded worker pool owns one value per worker. Keeping process and
/// runtime ownership here makes cleanup independent of command error paths.
pub struct NativeGeometer {
    executable: PathBuf,
    runtime: Runtime,
    client: Option<GeometerClient>,
}

impl NativeGeometer {
    pub fn discover() -> Result<PathBuf, NativeGeometerError> {
        #[cfg(feature = "embedded-geometer")]
        {
            std::env::current_exe().map_err(NativeGeometerError::CurrentExecutable)
        }
        #[cfg(not(feature = "embedded-geometer"))]
        {
            GeometerClient::find_executable().ok_or(NativeGeometerError::NotFound)
        }
    }

    pub fn connect_discovered() -> Result<Self, NativeGeometerError> {
        Self::connect(Self::discover()?)
    }

    pub fn connect(executable: impl AsRef<Path>) -> Result<Self, NativeGeometerError> {
        let executable = executable.as_ref().to_path_buf();
        let runtime = Runtime::new().map_err(NativeGeometerError::Runtime)?;
        let client = runtime.block_on(GeometerClient::spawn(
            &executable,
            "kicad-cruncher",
            env!("CARGO_PKG_VERSION"),
        ))?;
        let welcome = client.welcome();
        if welcome.release_version != GEOMETER_RELEASE
            || welcome.c_abi_generation != GEOMETER_C_ABI_GENERATION
        {
            let error = NativeGeometerError::Incompatible {
                release: welcome.release_version.clone(),
                c_abi_generation: welcome.c_abi_generation,
            };
            let _ = runtime.block_on(client.close());
            return Err(error);
        }
        Ok(Self {
            executable,
            runtime,
            client: Some(client),
        })
    }

    pub fn executable(&self) -> &Path {
        &self.executable
    }

    pub fn client(&self) -> &GeometerClient {
        self.client
            .as_ref()
            .expect("connected Geometer client is present")
    }

    pub fn illustrate_model(
        &self,
        request: ModelIllustrationGeometryRequestA0,
        step: Vec<u8>,
    ) -> Result<ModelIllustrationGeometry, NativeGeometerError> {
        Ok(self.runtime.block_on(
            self.client()
                .model_illustration_geometry(request, Some(step)),
        )?)
    }

    pub fn close(mut self) -> Result<(), NativeGeometerError> {
        if let Some(client) = self.client.take() {
            self.runtime.block_on(client.close())?;
        }
        Ok(())
    }
}

impl Drop for NativeGeometer {
    fn drop(&mut self) {
        if let Some(client) = self.client.take() {
            let _ = self.runtime.block_on(client.close());
        }
    }
}
