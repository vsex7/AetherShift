pub mod codec;
pub mod error;
pub mod path;
pub mod types;

pub use codec::{
    ClientStream, MAX_FRAME_LENGTH, ProtocolStream, ServerStream, call, new_length_delimited_codec,
    read_request, read_response, send_request, send_response,
};
pub use error::ProtocolError;
pub use path::{DEFAULT_SOCKET_NAME, default_socket_path};
pub use types::{
    BindingInfo, ConflictPolicy, DiagnosticCheck, DiagnosticStatus, DoctorReport, Event,
    HistoryEntry, LayoutFeedback, MetricsFormat, MetricsReport, PluginInfo, PluginPermissionScope,
    Recommendation, Request, Response, SnapLayout, StatusInfo, UsageStats, WindowPolicy,
};
