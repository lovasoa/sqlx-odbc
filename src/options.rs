use crate::{OdbcConnection, Result};
use log::LevelFilter;
use std::fmt::{self, Debug, Formatter};
use std::str::FromStr;
use std::time::Duration;
use url::Url;

/// Fetch-buffer settings used by the ODBC driver.
///
/// `max_column_size = Some(_)` enables buffered fetching and can truncate long text or binary
/// fields to the configured size. `max_column_size = None` keeps fetching unbuffered so variable
/// sized values are not truncated by this crate's buffer allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OdbcBufferSettings {
    /// Number of rows fetched in each batch.
    pub batch_size: usize,
    /// Maximum text or binary column size in buffered mode, or `None` for unbuffered mode.
    pub max_column_size: Option<usize>,
}

impl Default for OdbcBufferSettings {
    fn default() -> Self {
        Self {
            batch_size: 64,
            max_column_size: None,
        }
    }
}

/// Connection options for an ODBC data source.
#[derive(Clone)]
pub struct OdbcConnectOptions {
    pub(crate) conn_str: String,
    pub(crate) buffer_settings: OdbcBufferSettings,
    pub(crate) statement_cache_capacity: usize,
    pub(crate) log_statements: LevelFilter,
    pub(crate) log_slow_statements: LevelFilter,
    pub(crate) log_slow_statement_duration: Duration,
}

impl OdbcConnectOptions {
    /// Returns the normalized ODBC connection string.
    pub fn connection_string(&self) -> &str {
        &self.conn_str
    }

    /// Sets the buffer configuration for this connection.
    pub fn buffer_settings(&mut self, settings: OdbcBufferSettings) -> &mut Self {
        assert!(settings.batch_size > 0, "batch_size must be greater than 0");
        if let Some(size) = settings.max_column_size {
            assert!(size > 0, "max_column_size must be greater than 0");
        }

        self.buffer_settings = settings;
        self
    }

    /// Returns the current buffer settings.
    pub fn buffer_settings_ref(&self) -> &OdbcBufferSettings {
        &self.buffer_settings
    }

    /// Sets the number of rows fetched in each batch.
    pub fn batch_size(&mut self, batch_size: usize) -> &mut Self {
        assert!(batch_size > 0, "batch_size must be greater than 0");
        self.buffer_settings.batch_size = batch_size;
        self
    }

    /// Sets the maximum buffered column size, or `None` for unbuffered fetching.
    pub fn max_column_size(&mut self, max_column_size: Option<usize>) -> &mut Self {
        if let Some(size) = max_column_size {
            assert!(size > 0, "max_column_size must be greater than 0");
        }

        self.buffer_settings.max_column_size = max_column_size;
        self
    }

    /// Sets the maximum number of prepared statements kept in this connection's cache.
    pub fn statement_cache_capacity(&mut self, capacity: usize) -> &mut Self {
        self.statement_cache_capacity = capacity;
        self
    }

    /// Sets regular statement logging level.
    pub fn log_statements(&mut self, level: LevelFilter) -> &mut Self {
        self.log_statements = level;
        self
    }

    /// Sets slow statement logging level and threshold.
    pub fn log_slow_statements(&mut self, level: LevelFilter, duration: Duration) -> &mut Self {
        self.log_slow_statements = level;
        self.log_slow_statement_duration = duration;
        self
    }

    /// Opens a blocking ODBC connection.
    ///
    /// The full SQLx async connection/executor API will be layered on top of this during the port.
    pub fn connect_blocking(&self) -> Result<OdbcConnection> {
        OdbcConnection::connect_blocking(self)
    }
}

impl Debug for OdbcConnectOptions {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("OdbcConnectOptions")
            .field("conn_str", &"<redacted>")
            .field("buffer_settings", &self.buffer_settings)
            .field("statement_cache_capacity", &self.statement_cache_capacity)
            .field("log_statements", &self.log_statements)
            .field("log_slow_statements", &self.log_slow_statements)
            .field(
                "log_slow_statement_duration",
                &self.log_slow_statement_duration,
            )
            .finish()
    }
}

impl FromStr for OdbcConnectOptions {
    type Err = sqlx_core::Error;

    fn from_str(input: &str) -> std::result::Result<Self, Self::Err> {
        let mut trimmed = input.trim();

        if let Some(rest) = trimmed.strip_prefix("odbc:") {
            trimmed = rest;
        }

        let conn_str = if trimmed.contains('=') {
            trimmed.to_owned()
        } else {
            format!("DSN={trimmed}")
        };

        Ok(Self {
            conn_str,
            buffer_settings: OdbcBufferSettings::default(),
            statement_cache_capacity: 100,
            log_statements: LevelFilter::Debug,
            log_slow_statements: LevelFilter::Warn,
            log_slow_statement_duration: Duration::from_secs(1),
        })
    }
}

impl sqlx_core::connection::ConnectOptions for OdbcConnectOptions {
    type Connection = OdbcConnection;

    fn from_url(url: &Url) -> std::result::Result<Self, sqlx_core::Error> {
        Self::from_str(url.as_str())
    }

    async fn connect(&self) -> std::result::Result<Self::Connection, sqlx_core::Error> {
        self.connect_blocking().map_err(Into::into)
    }

    fn log_statements(mut self, level: LevelFilter) -> Self {
        self.log_statements = level;
        self
    }

    fn log_slow_statements(mut self, level: LevelFilter, duration: Duration) -> Self {
        self.log_slow_statements = level;
        self.log_slow_statement_duration = duration;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bare_dsn_as_dsn_connection_string() {
        let options = OdbcConnectOptions::from_str("Warehouse").unwrap();
        assert_eq!(options.connection_string(), "DSN=Warehouse");
    }

    #[test]
    fn preserves_standard_connection_strings() {
        let input = "Driver={ODBC Driver 17 for SQL Server};Server=localhost;Database=test";
        let options = OdbcConnectOptions::from_str(input).unwrap();
        assert_eq!(options.connection_string(), input);
    }

    #[test]
    fn strips_legacy_odbc_prefix() {
        let options = OdbcConnectOptions::from_str("odbc:DSN=Warehouse").unwrap();
        assert_eq!(options.connection_string(), "DSN=Warehouse");
    }

    #[test]
    fn updates_buffer_settings_incrementally() {
        let mut options = OdbcConnectOptions::from_str("Warehouse").unwrap();
        options.batch_size(128).max_column_size(Some(2048));

        assert_eq!(
            *options.buffer_settings_ref(),
            OdbcBufferSettings {
                batch_size: 128,
                max_column_size: Some(2048)
            }
        );
    }
}
