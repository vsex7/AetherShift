pub mod codec;
pub mod error;
pub mod path;
pub mod types;

pub use codec::{
    call, new_length_delimited_codec, read_request, read_response, send_request, send_response, ClientStream,
    ProtocolStream, ServerStream, MAX_FRAME_LENGTH,
};
pub use error::ProtocolError;
pub use path::{default_socket_path, DEFAULT_SOCKET_NAME};
pub use types::{
    ConflictPolicy, DiagnosticCheck, DiagnosticStatus, DoctorReport, Event, HistoryEntry, MetricsFormat,
    MetricsReport, PluginInfo, PluginPermissionScope, Recommendation, Request, Response, SnapLayout, StatusInfo,
    UsageStats, WindowPolicy, BindingInfo, LayoutFeedback,
};
