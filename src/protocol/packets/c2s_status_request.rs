use crate::protocol::packets::Packet;

pub struct C2SStatusRequest;

impl C2SStatusRequest {
    pub const ID: i32 = 0x00;
}

impl Packet for C2SStatusRequest {
    const ID: i32 = Self::ID;
}