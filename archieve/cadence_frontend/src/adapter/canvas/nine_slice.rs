#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NineSliceMargins {
    pub left: u32,
    pub right: u32,
    pub top: u32,
    pub bottom: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NineSliceFrame {
    pub key: &'static str,
    pub margins: NineSliceMargins,
}
