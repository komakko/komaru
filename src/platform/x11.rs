use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ClientMessageEvent, ConnectionExt, EventMask, Window,
};
use x11rb::rust_connection::RustConnection;

use crate::error::PlatformError;
use crate::platform::{TopmostController, TopmostState};

const STATE_REMOVE: u32 = 0;
const STATE_ADD: u32 = 1;
const SOURCE_APPLICATION: u32 = 1;
const MAX_STATE_ATOMS: u32 = 64;

pub struct X11Topmost {
    connection: RustConnection,
    root: Window,
    window: Window,
    wm_state: Atom,
    state_above: Atom,
}

impl X11Topmost {
    pub fn connect(window: Window) -> Result<Self, PlatformError> {
        let (connection, screen_number) = RustConnection::connect(None)?;
        let root = connection.setup().roots[screen_number].root;
        let wm_state = intern(&connection, b"_NET_WM_STATE")?;
        let state_above = intern(&connection, b"_NET_WM_STATE_ABOVE")?;
        Ok(Self {
            connection,
            root,
            window,
            wm_state,
            state_above,
        })
    }
}

impl TopmostController for X11Topmost {
    fn read(&self) -> Result<TopmostState, PlatformError> {
        let reply = self
            .connection
            .get_property(
                false,
                self.window,
                self.wm_state,
                AtomEnum::ATOM,
                0,
                MAX_STATE_ATOMS,
            )?
            .reply()?;
        let above = reply
            .value32()
            .is_some_and(|mut atoms| atoms.any(|atom| atom == self.state_above));
        Ok(if above {
            TopmostState::On
        } else {
            TopmostState::Off
        })
    }

    fn write(&self, enabled: bool) -> Result<(), PlatformError> {
        let action = if enabled { STATE_ADD } else { STATE_REMOVE };
        let event = ClientMessageEvent::new(
            32,
            self.window,
            self.wm_state,
            [action, self.state_above, 0, SOURCE_APPLICATION, 0],
        );
        let mask = EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;
        self.connection.send_event(false, self.root, mask, event)?;
        self.connection.flush()?;
        Ok(())
    }
}

fn intern(connection: &RustConnection, name: &[u8]) -> Result<Atom, PlatformError> {
    Ok(connection.intern_atom(false, name)?.reply()?.atom)
}
