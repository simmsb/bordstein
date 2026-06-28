pub mod layer;
pub mod status_bar;
pub mod text;

pub use layer::{Layer, LayerMut, LayerRef};
pub use status_bar::StatusBarLayer;
pub use text::TextLayer;

pub trait IsLayer {
    fn layer<'a>(&'a self) -> LayerRef<'a>;
    fn layer_mut<'a>(&'a mut self) -> LayerMut<'a>;
}

pub trait AsChildLayer<'a>: Sized + IsLayer {
    type Parameters;

    fn new_unparented(create_params: Self::Parameters) -> Option<Self>;
}
