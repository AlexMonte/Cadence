#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpriteRegion {
    pub key: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SpriteAtlas {
    pub frame_keys: Vec<&'static str>,
}
