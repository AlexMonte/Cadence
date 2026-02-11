use bevy::prelude::*;
use grooveatlas::GrooveAtlasApp;

fn main() -> AppExit {
    if grooveatlas::runtime::is_runtime_host_mode() {
        grooveatlas::runtime::run_runtime_host_mode_or_exit();
    }

    App::new().add_plugins(GrooveAtlasApp).run()
}
