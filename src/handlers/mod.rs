pub mod static_files;
pub mod php_handler;
pub mod javascript_handler;
pub mod config_handler;
pub mod websocket_handler;

pub use static_files::serve_static_file;
pub use php_handler::handle_php;
pub use javascript_handler::handle_js;
pub use config_handler::{ get_config, update_config, reload_config };
pub use websocket_handler::websocket_handler;
